systemctl daemon-reload 2>/dev/null || :
systemctl reload dbus 2>/dev/null || :
if [ "$1" -eq 1 ]; then
    echo "Hearth is installed. Set it up with:  sudo hearthd setup"
elif systemctl is-active --quiet hearthd; then
    systemctl restart hearth-dns hearthd || :
fi
