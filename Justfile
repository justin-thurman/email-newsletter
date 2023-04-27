default:
    @just --list

up:
    ./bin/init_db.sh
    ./bin/init_redis.sh

build:
    docker build --tag email-newsletter --file Dockerfile .

run:
    docker run -p 8000:8000 email-newsletter

prepare-sqlx:
    cargo sqlx prepare -- --lib

do-create:
    doctl apps create --spec spec.yaml

do-update APP_ID:
    doctl apps update {{APP_ID}} --spec spec.yaml

do-list:
    doctl apps list
