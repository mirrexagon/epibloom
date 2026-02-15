check: check-server check-scripts check-ui

check-server:
    cd epibloom && cargo clippy

check-scripts:
    cd epibloom && lua-language-server --check=scripts

check-ui:
    cd epibloom/ui && npm run lint

run: copy-scripts
    cd epibloom && cargo run

copy-scripts:
    rsync -rvP epibloom/scripts ~/.config/epibloom

serve-ui:
    cd epibloom/ui && npm run dev
