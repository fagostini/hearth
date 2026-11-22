//! `hearthd dns`: the per-child filtering resolver. Runs as its own unprivileged
//! service (systemd DynamicUser), so DNS parsing never happens as root.
//! nftables (installed by hearthd, see `nft_rules`) redirects each child's port-53
//! traffic to that child's port; the port is how we know whose query it is.
use crate::config::{ChildRule, Config};
use hickory_proto::{
    op::{Message, ResponseCode},
    rr::{rdata::CNAME, Name, RData, Record},
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
};
use tracing::{info, warn};

/// Each child's resolver port. Derived from the UID so the resolver and the firewall
/// rules agree without sharing state.
pub fn port(uid: u32) -> u16 {
    40000 + (uid % 20000) as u16
}

/// Firefox checks this name and turns its own DNS-over-HTTPS off if it doesn't resolve.
const DOH_CANARY: &str = "use-application-dns.net";

pub struct Policy {
    pub user: String,
    pub block: Vec<String>,
    pub safesearch: bool,
}

pub enum Verdict {
    Forward,
    Block,
    Rewrite(&'static str),
}

/// Does `name` equal `domain` or sit under it? Both lower-case, no trailing dot.
fn under(name: &str, domain: &str) -> bool {
    name == domain || name.strip_suffix(domain).is_some_and(|p| p.ends_with('.'))
}

/// SafeSearch: the providers' own "forced safe" hostnames.
fn safesearch(name: &str) -> Option<&'static str> {
    let google = name.strip_prefix("www.").unwrap_or(name);
    if google.strip_prefix("google.").is_some_and(|tld| tld.split('.').count() <= 2) {
        return Some("forcesafesearch.google.com");
    }
    match name {
        "www.youtube.com" | "m.youtube.com" | "youtubei.googleapis.com" | "youtube.googleapis.com"
        | "www.youtube-nocookie.com" => Some("restrictmoderate.youtube.com"),
        "www.bing.com" | "bing.com" => Some("strict.bing.com"),
        "duckduckgo.com" | "www.duckduckgo.com" => Some("safe.duckduckgo.com"),
        _ => None,
    }
}

impl Policy {
    pub fn verdict(&self, name: &str) -> Verdict {
        let name = name.trim_end_matches('.').to_ascii_lowercase();
        if under(&name, DOH_CANARY) || self.block.iter().any(|d| under(&name, d)) {
            return Verdict::Block;
        }
        match self.safesearch.then(|| safesearch(&name)).flatten() {
            Some(target) => Verdict::Rewrite(target),
            None => Verdict::Forward,
        }
    }
}

/// Answer one query packet. None = drop (unparseable, or upstream silent).
async fn answer(policy: &Policy, upstream: SocketAddr, packet: &[u8]) -> Option<Vec<u8>> {
    let query = Message::from_vec(packet).ok()?;
    let q = query.queries.first()?.clone();
    match policy.verdict(&q.name().to_ascii()) {
        Verdict::Forward => forward(upstream, packet).await,
        Verdict::Block => {
            info!(user = policy.user, name = %q.name(), "blocked");
            let mut r = Message::response(query.metadata.id, query.metadata.op_code);
            r.metadata.recursion_desired = query.metadata.recursion_desired;
            r.metadata.recursion_available = true;
            r.metadata.response_code = ResponseCode::NXDomain;
            r.add_query(q);
            r.to_vec().ok()
        }
        Verdict::Rewrite(target) => {
            // Ask upstream for the target, then answer "name CNAME target" + its records.
            let target_name = Name::from_ascii(format!("{target}.")).ok()?;
            let mut sub = query.clone();
            let mut tq = q.clone();
            tq.set_name(target_name.clone());
            sub.queries = vec![tq];
            let mut r = Message::from_vec(&forward(upstream, &sub.to_vec().ok()?).await?).ok()?;
            r.queries = vec![q.clone()];
            let cname = Record::from_rdata(q.name().clone(), 300, RData::CNAME(CNAME(target_name)));
            r.answers.insert(0, cname);
            r.to_vec().ok()
        }
    }
}

async fn forward(upstream: SocketAddr, packet: &[u8]) -> Option<Vec<u8>> {
    let bind: SocketAddr = if upstream.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().ok()?;
    let sock = UdpSocket::bind(bind).await.ok()?;
    sock.send_to(packet, upstream).await.ok()?;
    let mut buf = vec![0u8; 4096];
    let (n, from) = tokio::time::timeout(Duration::from_secs(4), sock.recv_from(&mut buf)).await.ok()?.ok()?;
    (from == upstream).then(|| {
        buf.truncate(n);
        buf
    })
}

/// First nameserver in resolv.conf (on systemd-resolved systems, its stub 127.0.0.53).
pub fn upstream(resolv_conf: &str) -> Option<SocketAddr> {
    resolv_conf.lines().find_map(|l| {
        let ip = l.trim().strip_prefix("nameserver")?.trim();
        Some(SocketAddr::new(ip.split('%').next()?.parse().ok()?, 53))
    })
}

async fn serve(policy: Arc<Policy>, port: u16, upstream: SocketAddr) -> anyhow::Result<()> {
    for ip in ["127.0.0.1", "::1"] {
        let addr = SocketAddr::new(ip.parse()?, port);
        let udp = Arc::new(UdpSocket::bind(addr).await?);
        let p = policy.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                let Ok((n, from)) = udp.recv_from(&mut buf).await else { continue };
                let (udp, p, pkt) = (udp.clone(), p.clone(), buf[..n].to_vec());
                tokio::spawn(async move {
                    if let Some(r) = answer(&p, upstream, &pkt).await {
                        let _ = udp.send_to(&r, from).await;
                    }
                });
            }
        });
        let tcp = TcpListener::bind(addr).await?;
        let p = policy.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = tcp.accept().await else { continue };
                let p = p.clone();
                tokio::spawn(async move {
                    let mut len = [0u8; 2];
                    // ponytail: one query per TCP connection; pipelining if a client ever needs it.
                    let read = async {
                        s.read_exact(&mut len).await?;
                        let mut pkt = vec![0u8; u16::from_be_bytes(len) as usize];
                        s.read_exact(&mut pkt).await?;
                        std::io::Result::Ok(pkt)
                    };
                    let Ok(Ok(pkt)) = tokio::time::timeout(Duration::from_secs(5), read).await else { return };
                    if let Some(r) = answer(&p, upstream, &pkt).await {
                        let _ = s.write_all(&(r.len() as u16).to_be_bytes()).await;
                        let _ = s.write_all(&r).await;
                    }
                });
            }
        });
    }
    Ok(())
}

pub async fn run(_cfg: Config, children: Vec<(ChildRule, u32)>) -> anyhow::Result<()> {
    let up = upstream(&std::fs::read_to_string("/etc/resolv.conf")?)
        .ok_or_else(|| anyhow::anyhow!("no nameserver in /etc/resolv.conf"))?;
    for (r, uid) in children {
        let Some(web) = r.web else { continue };
        let policy = Arc::new(Policy {
            user: r.user.clone(),
            block: web.block.iter().map(|d| d.trim_end_matches('.').to_ascii_lowercase()).collect(),
            safesearch: web.safesearch,
        });
        serve(policy, port(uid), up).await?;
        info!(user = r.user, uid, port = port(uid), %up, "resolver listening");
    }
    std::future::pending::<()>().await;
    Ok(())
}

/// nftables ruleset for children with web rules: redirect their DNS (any server, UDP
/// and TCP, IPv4 and IPv6) to their resolver port, and reject DNS-over-TLS.
/// Applied atomically; replaces any previous Hearth table.
pub fn nft_rules(uids: &[u32]) -> String {
    let mut nat = String::new();
    let mut filter = String::new();
    for &u in uids {
        nat += &format!("    meta skuid {u} meta l4proto {{ tcp, udp }} th dport 53 redirect to :{}\n", port(u));
        filter += &format!("    meta skuid {u} meta l4proto {{ tcp, udp }} th dport 853 reject\n");
    }
    format!(
        "table inet hearth\ndelete table inet hearth\ntable inet hearth {{\n  chain dns {{\n    type nat hook output priority -100; policy accept;\n{nat}  }}\n  chain block {{\n    type filter hook output priority 0; policy accept;\n{filter}  }}\n}}\n"
    )
}

pub async fn apply_nft(uids: &[u32]) {
    use tokio::process::Command;
    let rules = nft_rules(uids);
    let child = Command::new("nft").args(["-f", "-"]).stdin(std::process::Stdio::piped()).spawn();
    let res = async {
        let mut c = child?;
        c.stdin.take().unwrap().write_all(rules.as_bytes()).await?;
        let st = c.wait().await?;
        anyhow::ensure!(st.success(), "nft exited with {st}");
        anyhow::Ok(())
    };
    match res.await {
        Ok(()) => info!(children = uids.len(), "firewall rules applied"),
        Err(e) => warn!(%e, "firewall rules NOT applied: web filtering is off"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Policy {
        Policy { user: "a".into(), block: vec!["tiktok.com".into()], safesearch: true }
    }

    #[test]
    fn verdicts() {
        let v = |n| match p().verdict(n) {
            Verdict::Forward => "fwd".to_string(),
            Verdict::Block => "block".into(),
            Verdict::Rewrite(t) => t.into(),
        };
        assert_eq!(v("www.TikTok.com."), "block");
        assert_eq!(v("tiktok.com"), "block");
        assert_eq!(v("nottiktok.com"), "fwd");
        assert_eq!(v("use-application-dns.net."), "block");
        assert_eq!(v("www.google.com."), "forcesafesearch.google.com");
        assert_eq!(v("www.google.co.uk"), "forcesafesearch.google.com");
        assert_eq!(v("mail.google.com"), "fwd");
        assert_eq!(v("www.youtube.com"), "restrictmoderate.youtube.com");
        assert_eq!(v("example.org"), "fwd");
        let off = Policy { safesearch: false, ..p() };
        assert!(matches!(off.verdict("www.google.com"), Verdict::Forward));
    }

    #[test]
    fn helpers() {
        assert_eq!(port(1001), 41001);
        assert_eq!(upstream("# x\nnameserver 127.0.0.53\noptions edns0\n"), Some("127.0.0.53:53".parse().unwrap()));
        assert_eq!(upstream("nameserver fe80::1%eth0\n"), Some("[fe80::1]:53".parse().unwrap()));
        let r = nft_rules(&[1001]);
        assert!(r.contains("meta skuid 1001 meta l4proto { tcp, udp } th dport 53 redirect to :41001"));
        assert!(r.contains("dport 853 reject"));
    }

    #[tokio::test]
    async fn block_answer() {
        let mut q = Message::new(7, hickory_proto::op::MessageType::Query, hickory_proto::op::OpCode::Query);
        q.add_query(hickory_proto::op::Query::query(
            Name::from_ascii("www.tiktok.com.").unwrap(),
            hickory_proto::rr::RecordType::A,
        ));
        let r = answer(&p(), "127.0.0.1:9".parse().unwrap(), &q.to_vec().unwrap()).await.unwrap();
        let r = Message::from_vec(&r).unwrap();
        assert_eq!((r.metadata.id, r.metadata.response_code), (7, ResponseCode::NXDomain));
    }
}
