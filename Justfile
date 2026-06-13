check: check-server check-ui

check-server:
    cd epibloom && cargo clippy

run-server: copy-scripts
    cd epibloom && cargo run

copy-scripts:
    rsync -rvP --delete epibloom/scripts ~/.config/epibloom

check-ui:
    cd epibloom/ui && npm run lint

serve-ui:
    cd epibloom/ui && npm run dev
