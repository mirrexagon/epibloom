check: check-server check-scripts check-ui

check-server:
    cd epibloom && cargo clippy

check-scripts:
    fd -0 -tfile '\.lua$|\.tl$' epibloom/scripts | xargs -0 lua epibloom/deps/tl/tl check

check-ui:
    cd epibloom/ui && npm run lint

run: copy-scripts
    cd epibloom && cargo run

copy-scripts:
    fd -0 -tfile '\.tl$' epibloom/scripts | xargs -0 -I{} sh -c 'lua epibloom/deps/tl/tl gen -o "$(dirname "$1")/$(basename "$1" .tl).lua" "$1"' _ {}
    rsync -rvP --delete epibloom/scripts ~/.config/epibloom

serve-ui:
    cd epibloom/ui && npm run dev
