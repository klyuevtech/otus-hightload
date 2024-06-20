# Домашнее задание
## Отказоустойчивость приложений

# Настройка nginx

Конфигурация узла nginx описана в файлах `Dockerfile` и `nginx.conf` в папке `dialogs/proxy`:

Dockerfile
```dockerfile
FROM nginx
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY server.crt /etc/nginx/ssl/dialogs/server.crt
COPY server.key /etc/nginx/ssl/dialogs/server.key
```

nginx.conf
```conf
upstream dialogs {# Defaults to round_robin
    server dialogs01:8000 weight=5 max_fails=3 fail_timeout=10s;
    server dialogs02:8000 weight=5 max_fails=3 fail_timeout=10s;
}

server {
    listen 8000 ssl;
    ssl_certificate         /etc/nginx/ssl/dialogs/server.crt;
    ssl_certificate_key     /etc/nginx/ssl/dialogs/server.key;

    location / {
        proxy_pass https://dialogs;
    }
}
```

При билде Docker образа с nginx мы копируем файл конфигурации и сертификаты.
В секции upstream указываем адреса серверов приложений объединенные в серверную группу `dialogs`. Load‑balancing алгоритм не указываем, таким образом по умолчанию будет использоваться `Round Robin`.
В секции server указываем пути к сертификатам. Даем инструкцию слушать порт 8000 и использовать ssl. Указываем перенаправлять все запросы на адрес `https://dialogs` который ссылается на серверную группу `dialogs`.
В файле `docker-compose-patroni.yaml` указываем 2 экземпляра приложения с разными именами хостов: `dialogs01` и `dialogs02` .


# Настройка HAProxy

Воспользуемся HA Темплейтом на базе Patroni, который включает в себя Postgres, HAProxy и etcd. Следуем инструкции: 
https://github.com/patroni/patroni/blob/master/docker-compose.yml

```text
# requires a patroni image build from the Dockerfile:
# $ docker build -t patroni .
# The cluster could be started as:
# $ docker-compose up -d
```

Переносим нужные нам сервисы в `docker-compose-patroni.yaml` и подключаем их к сети `backend`.
Экземпляры приложений подключаем к `haproxy:5000` для записи и к `haproxy:5001` для чтения.

docker-compose-patroni.yaml
```yaml
networks:
  backend:
  server-side:
    name: server-side

services:
  dialogs:
    build:
      context: ./proxy
    depends_on:
      - dialogs01
      - dialogs02
    networks:
      - server-side

  dialogs01:
    build:
      context: .
      target: development
    environment:
      - HTTP_SERVER_ADDRESS=0.0.0.0:8000
      - RUST_LOG=debug
      - PG_DBNAME=postgres
      - PG_AUTHORITY_MASTER=haproxy:5000
      - PG_AUTHORITY_REPLICA=haproxy:5001
      - PG_USER=postgres
      - PG_PASSWORD=postgres
      - PG_MASTER_POOL_MAX_SIZE=100
      - PG_REPLICA_POOL_MAX_SIZE=300
      - SELF_HOST_NAME=dialogs01
    networks: [ backend, server-side ]

    # ports:
      # - 8001:8000
      # - 9000:9000
    volumes:
      - ./src:/code/src
      - backend-cache:/code/target
    depends_on:
      - haproxy

  dialogs02:
    build:
      context: .
      target: development
    environment:
      - HTTP_SERVER_ADDRESS=0.0.0.0:8000
      - RUST_LOG=debug
      - PG_DBNAME=postgres
      - PG_AUTHORITY_MASTER=haproxy:5000
      - PG_AUTHORITY_REPLICA=haproxy:5001
      - PG_USER=postgres
      - PG_PASSWORD=postgres
      - PG_MASTER_POOL_MAX_SIZE=100
      - PG_REPLICA_POOL_MAX_SIZE=300
      - SELF_HOST_NAME=dialogs02
    networks: [ backend, server-side ]

    # ports:
      # - 8001:8000
      # - 9000:9000
    volumes:
      - ./src:/code/src
      - backend-cache:/code/target
    depends_on:
      - haproxy

  etcd1: &etcd
      image: ${PATRONI_TEST_IMAGE:-patroni}
      networks: [ backend ]
      environment:
          ETCD_LISTEN_PEER_URLS: http://0.0.0.0:2380
          ETCD_LISTEN_CLIENT_URLS: http://0.0.0.0:2379
          ETCD_INITIAL_CLUSTER: etcd1=http://etcd1:2380,etcd2=http://etcd2:2380,etcd3=http://etcd3:2380
          ETCD_INITIAL_CLUSTER_STATE: new
          ETCD_INITIAL_CLUSTER_TOKEN: tutorial
          ETCD_UNSUPPORTED_ARCH: arm64
      container_name: backend-etcd1
      hostname: etcd1
      command: etcd --name etcd1 --initial-advertise-peer-urls http://etcd1:2380

  etcd2:
      <<: *etcd
      container_name: backend-etcd2
      hostname: etcd2
      command: etcd --name etcd2 --initial-advertise-peer-urls http://etcd2:2380

  etcd3:
      <<: *etcd
      container_name: backend-etcd3
      hostname: etcd3
      command: etcd --name etcd3 --initial-advertise-peer-urls http://etcd3:2380

  haproxy:
      image: ${PATRONI_TEST_IMAGE:-patroni}
      networks: [ backend ]
      env_file: docker/patroni.env
      hostname: haproxy
      container_name: backend-haproxy
      # ports:
      #     - "5000:5000"
      #     - "5001:5001"
      command: haproxy
      environment: &haproxy_env
          ETCDCTL_ENDPOINTS: http://etcd1:2379,http://etcd2:2379,http://etcd3:2379
          PATRONI_ETCD3_HOSTS: "'etcd1:2379','etcd2:2379','etcd3:2379'"
          PATRONI_SCOPE: backend

  patroni1:
      image: ${PATRONI_TEST_IMAGE:-patroni}
      networks: [ backend ]
      env_file: docker/patroni.env
      hostname: patroni1
      container_name: backend-patroni1
      environment:
          <<: *haproxy_env
          PATRONI_NAME: patroni1

  patroni2:
      image: ${PATRONI_TEST_IMAGE:-patroni}
      networks: [ backend ]
      env_file: docker/patroni.env
      hostname: patroni2
      container_name: backend-patroni2
      environment:
          <<: *haproxy_env
          PATRONI_NAME: patroni2

  patroni3:
      image: ${PATRONI_TEST_IMAGE:-patroni}
      networks: [ backend ]
      env_file: docker/patroni.env
      hostname: patroni3
      container_name: backend-patroni3
      environment:
          <<: *haproxy_env
          PATRONI_NAME: patroni3


volumes:
  backend-cache: {}
  healthcheck-volume:
```

## Запуск приложения

Микросервис:
```bash
docker build -t patroni .patroni/
docker compose -f dialogs/docker-compose-patroni.yaml up -d --force-recreate 
```

Шлюз (роль которого пока выполняет старый монолит):
```bash
docker compose -f .docker/docker-compose.yaml  up -d --force-recreate 
```

Экземпляры `dialog0?` могут запустится не сразу. Так получается что они стартуют быстрее чем становится доступна база данных, это можно поправить, руки не дошли. Пока можно просто запустить их повторно через docker desktop например.

## Проверка балансировки

Для того, чтобы понимать на какой из экземпляров приложения попал отправленный запрос, я добавил отправку сервером http заголовка `x-server-instance` который содержит host name узла.

dialogs/src/main.rs
```rust
    HttpResponse::Ok()
        .insert_header(("x-request-id", x_request_id))
        .insert_header(("x-server-instance", std::env::var("SELF_HOST_NAME").unwrap_or(String::from("unknown"))))
        .json(
            serde_json::json!(
                dialog::Dialog::list_messages(&**client, user_id1, user_id2, offset, limit)
                    .await
                    .unwrap_or_else(|e| {log::debug!("Unable to get messages: {:?}", e);return Vec::new();})
            )
        )
```


Отправляем запросы на шлюз и смотрим на значение заголовка `x-server-instance`. Видно, что значение время от времени меняется с `dialogs01` на `dialogs02` и обратно.

## Проверка надежности

Для проверки надежности останавливаем контейнеры например `dialogs02-1` (один из экземпляров приложения на которое перенаправляет запросы nginx proxy) и `backend-patroni2` (одна из реплик, на которую перенаправляет запросы HAProxy). Продолжаем отправлять запросы с Postman на адрес шлюза, видим что запросы продолжаю возвращать результат.