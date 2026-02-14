check:
    cd epibloom && cargo clippy

check-ui:
    cd epibloom/ui && npm run lint

run: copy-scripts
    cd epibloom && cargo run

copy-scripts:
    rsync -rvP epibloom/scripts/ ~/.config/epibloom

serve:
    cd epibloom/ui && npm run dev
