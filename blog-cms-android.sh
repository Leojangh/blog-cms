#!/system/bin/sh
DIR="/mnt/scratch/overlay/system_ext/upper/blog-cms"
BIN="$DIR/blog-cms"
PIDFILE="$DIR/blog-cms.pid"
ENV="$DIR/.env"
LOG="$DIR/blog-cms.log"
RESTART_SEC=5

load_env() {
    [ -f "$ENV" ] && while IFS= read -r line; do
        case "$line" in \#*|"") continue ;; esac
        export "$line"
    done < "$ENV"
}

get_pid() {
    [ -f "$PIDFILE" ] && read pid < "$PIDFILE" && kill -0 "$pid" 2>/dev/null && echo "$pid"
}

do_start() {
    if old=$(get_pid); then
        echo "already running (pid $old)"
        return 1
    fi
    load_env
    cd "$DIR"
    nohup "$BIN" >> "$LOG" 2>&1 &
    echo $! > "$PIDFILE"
    echo "started (pid $!)"
}

do_stop() {
    pid=$(get_pid) || { echo "not running"; return 1; }
    kill "$pid" && rm -f "$PIDFILE"
    echo "stopped (pid $pid)"
}

do_restart() { do_stop 2>/dev/null; sleep 1; do_start; }

do_status() {
    if pid=$(get_pid); then
        echo "running (pid $pid)"
    else
        rm -f "$PIDFILE"
        echo "stopped"
        return 1
    fi
}

do_watch() {
    echo "watching with restart-on-failure (interval ${RESTART_SEC}s)..."
    trap 'do_stop 2>/dev/null; exit 0' INT TERM
    while true; do
        if ! get_pid >/dev/null; then
            echo "$(date): process died, restarting..."
            do_start
        fi
        sleep $RESTART_SEC
    done
}

case "$1" in
    start)   do_start ;;
    stop)    do_stop ;;
    restart) do_restart ;;
    status)  do_status ;;
    watch)   do_start 2>/dev/null; do_watch ;;
    log)     tail -f "$LOG" ;;
    *)       echo "usage: $0 {start|stop|restart|status|watch|log}" ;;
esac
