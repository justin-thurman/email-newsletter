default:
    @just --list

build:
    docker build --tag email-newsletter --file Dockerfile .

prepare-sqlx:
    cargo sqlx prepare -- --lib
