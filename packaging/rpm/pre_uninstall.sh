# $1 = 0: removal, not upgrade. Drop the children's DNS redirect too, or it would
# point at a resolver that's no longer there and they'd have no DNS at all.
if [ "$1" -eq 0 ]; then
    systemctl disable --now hearthd hearth-dns 2>/dev/null || :
    nft delete table inet hearth 2>/dev/null || :
fi
