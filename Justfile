default:
    @just --list

build:
    docker build --tag email-newsletter --file Dockerfile .

run:
    docker run -p 8000:8000 email-newsletter

prepare-sqlx:
    cargo sqlx prepare -- --lib
