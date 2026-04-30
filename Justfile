check: check-server check-scripts check-ui

check-server:
    cd epibloom && cargo clippy

check-scripts:
    cd epibloom && fd -0 -tfile '\.lua$|\.tl$' scripts | xargs -0 lua deps/tl/tl check

check-ui:
    cd epibloom/ui && npm run lint

run: copy-scripts
    cd epibloom && cargo run

copy-scripts:
    rsync -rvP epibloom/scripts ~/.config/epibloom

serve-ui:
    cd epibloom/ui && npm run dev
