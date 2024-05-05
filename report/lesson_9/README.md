В целях обучения репликация была настроена 3-мя способами:
 - Простая конфигурация с тремя postgres в трех контейнерах запускаемых при помощи docker compose. Директория .docker
 - Та же конфигурация только с использованием кластера minikube. Директория .kube
 - HA решение на базе patroni. Директория .patroni

Для пунктов 1-3 задания используем .docker конфигурацию. Для подключения к базам данных на запись, приложение backend использует переменную окружения PG_AUTHORITY_MASTER:
 - PG_AUTHORITY_MASTER=db:5432

Для подключения к базе данных на чтение используется переменная окружения PG_AUTHORITY_REPLICA:
 - PG_AUTHORITY_REPLICA=dbreplica1:5432

Значения этих переменных окружения в конфигурации .docker задаются в файле docker-compose.yaml.
Поэтому чтобы переключить приложение backend на чтение в мастера достаточно поменять значение PG_AUTHORITY_REPLICA на db:5432 таким образом:
 - PG_AUTHORITY_REPLICA=db:5432

И перезапустить контейнеры:
```bash
cd .docker
docker compose -f ./docker-compose.yaml up -d --force-recreate
```
Запускаем Jmeter и получаем результат:
![](./assets/master_only/Screenshot%202024-05-05%20at%2010.49.37%20(2).png)
![](./assets/master_only/Screenshot%202024-05-05%20at%2010.49.37.png)

Теперь возвращаем PG_AUTHORITY_REPLICA старое значение dbreplica1:5432 и перезапускаем контейнеры. Подаем нагрузку и смотрим результат:
![](./assets/one_replica/Screenshot%202024-05-05%20at%2010.53.27%20(2).png)
![](./assets/one_replica/Screenshot%202024-05-05%20at%2010.53.27.png)

Для следующих пунктов задания используем patroni.
Как запустить patroni кластер подробно описано в .patroni/docker/README.md . Если кратко, то таким образом:

```bash
cd .patroni/
docker build -t patroni .
docker compose -f ./docker-compose.yml up -d
```

Если контейнер backend не запуститься автоматически, его нужно будет запустить вручную после старта остальных контейнеров узла. Это можно исправить, но руки не дошли.

По умолчанию реплики в данной конфигурации асинхронные. Для включения одной снихронной реплики в конфигурации с patroni достаточно в файлах .patroni/postgres*.yml сделать изменение:
```bash
%  git diff postgres0.yml
diff --git a/postgres0.yml b/postgres0.yml
index 1232ab4..ad261ef 100644
--- a/postgres0.yml
+++ b/postgres0.yml
@@ -56,7 +56,7 @@ bootstrap:
     retry_timeout: 10
     maximum_lag_on_failover: 1048576
 #    primary_start_timeout: 300
-#    synchronous_mode: false
+    synchronous_mode: true
     #standby_cluster:
       #host: 127.0.0.1
       #port: 1111
@@ -72,6 +72,8 @@ bootstrap:
       #  - hostssl all all 0.0.0.0/0 md5
 #      use_slots: true
       parameters:
+        synchronous_commit: "on"
+        synchronous_standby_names: "*"
 #        wal_level: hot_standby
 #        hot_standby: "on"
 #        max_connections: 100
 ```

И сделать build образа и заново запустить из него контейнер как описано выше.

Для того, чтобы посмотреть статистику репликации, нужно понять какой именно нод захватил lock и используется в данный момент как мастер. Для этого нужно посмотреть логи какого-нибудь из demo-patroni* контейнеров, например:

```bash
% docker ps               
CONTAINER ID   IMAGE             COMMAND                  CREATED          STATUS          PORTS                                            NAMES
0ec5d45a1626   patroni-backend   "cargo run --offline"    35 minutes ago   Up 35 minutes   0.0.0.0:8000->8000/tcp, 0.0.0.0:9000->9000/tcp   patroni-backend-1
254e22e70078   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes                                                    demo-etcd3
25a9b05a1f44   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes                                                    demo-patroni1
54889cc13867   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes                                                    demo-patroni2
65302e61fc08   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes                                                    demo-etcd2
901c9b7a536f   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes                                                    demo-etcd1
4e5f919f5d0a   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes                                                    demo-patroni3
984361e09d91   patroni           "/bin/sh /entrypoint…"   35 minutes ago   Up 35 minutes   0.0.0.0:5001->5000/tcp, 0.0.0.0:5002->5001/tcp   demo-haproxy
% docker logs 25a9b05a1f44
2024-05-05 09:15:53,662 INFO: no action. I am (patroni1), a secondary, and following a leader (patroni2)
```

Из логов видно, что patroni2 у нас в данный момент является лидером (мастером).

Заходим внутрь контейнера любым способом, например:

```bash
.patroni % docker ps
CONTAINER ID   IMAGE             COMMAND                  CREATED          STATUS          PORTS                                            NAMES
0ec5d45a1626   patroni-backend   "cargo run --offline"    59 minutes ago   Up 59 minutes   0.0.0.0:8000->8000/tcp, 0.0.0.0:9000->9000/tcp   patroni-backend-1
254e22e70078   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes                                                    demo-etcd3
25a9b05a1f44   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes                                                    demo-patroni1
54889cc13867   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes                                                    demo-patroni2
65302e61fc08   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes                                                    demo-etcd2
901c9b7a536f   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes                                                    demo-etcd1
4e5f919f5d0a   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes                                                    demo-patroni3
984361e09d91   patroni           "/bin/sh /entrypoint…"   59 minutes ago   Up 59 minutes   0.0.0.0:5001->5000/tcp, 0.0.0.0:5002->5001/tcp   demo-haproxy
.patroni % docker exec -it 54889cc13867 psql
psql (16.2 (Debian 16.2-1.pgdg120+2))
Type "help" for help.

postgres=# select * from pg_stat_replication;

pid | usesysid |  usename   | application_name | client_addr | client_hostname | client_port |         backend_start         | backend_xmin |   state   | sent_lsn  | write_lsn | flush_lsn | replay_lsn | write_lag | flush_lag | replay_lag | sync_priority | sync_state |          reply_time           
-----+----------+------------+------------------+-------------+-----------------+-------------+-------------------------------+--------------+-----------+-----------+-----------+-----------+------------+-----------+-----------+------------+---------------+------------+-------------------------------
  61 |    16384 | replicator | patroni3         | 172.18.0.3  |                 |       38758 | 2024-05-05 08:41:34.590208+00 |              | streaming | 0/4115A08 | 0/4115A08 | 0/4115A08 | 0/4115A08  |           |           |            |             0 | async      | 2024-05-05 09:42:18.972748+00
  62 |    16384 | replicator | patroni1         | 172.18.0.2  |                 |       36572 | 2024-05-05 08:41:34.590587+00 |              | streaming | 0/4115A08 | 0/4115A08 | 0/4115A08 | 0/4115A08  |           |           |            |             1 | sync       | 2024-05-05 09:42:19.08288+00
(2 rows)

```

Видим, что у нас работают 2 реплики, одна из которых (patroni1) в данный момент синхронная.

Для того чтобы сделать обе реплики синхронными нужно поднять значение параметра конфигурации synchronous_node_count patroni в postgres*.yml до 2:
```yaml
synchronous_node_count: 2
```

Перевсобираем и перезапускаем контейнеры и смотрим статистику репликации мастера:

```bash
 pid | usesysid |  usename   | application_name | client_addr | client_hostname | client_port |         backend_start         | backend_xmin |   state   | sent_lsn  | write_lsn | flush_lsn | replay_lsn | write_lag | flush_lag | replay_lag | sync_priority | sync_state |          reply_time           
-----+----------+------------+------------------+-------------+-----------------+-------------+-------------------------------+--------------+-----------+-----------+-----------+-----------+------------+-----------+-----------+------------+---------------+------------+-------------------------------
  59 |    16384 | replicator | patroni2         | 172.18.0.8  |                 |       53950 | 2024-05-05 09:47:06.224956+00 |              | streaming | 0/4115AF0 | 0/4115AF0 | 0/4115AF0 | 0/4115AF0  |           |           |            |             1 | sync       | 2024-05-05 10:04:29.207208+00
  62 |    16384 | replicator | patroni3         | 172.18.0.6  |                 |       58922 | 2024-05-05 09:47:09.756673+00 |              | streaming | 0/4115AF0 | 0/4115AF0 | 0/4115AF0 | 0/4115AF0  |           |           |            |             2 | sync       | 2024-05-05 10:04:29.216637+00
(2 rows)
```

Видим, что обе реплики у нас теперь синхронные.

Добавляем еще несколько реплик в конфигурацию. Для этого создаем дополнительные postgres*.yml и добавляем сервисы для них в docker-compose.yaml

Для создания нагрузки на запись я использовал Jmeter. Файл конфигурации Jmeter "Graph Results.jmx" можно найти в директории отчета.
Запускаем Jmeter с нагрузкой на запись (запросы POST user/register)

Теперь чтобы остановить одну из реплик, нужно понять кто мастер чтобы случайно не остановить его. Для этого выполняем:

```bash
.patroni % docker ps
CONTAINER ID   IMAGE             COMMAND                  CREATED          STATUS          PORTS                                            NAMES
cc22770d009f   patroni-backend   "cargo run --offline"    12 minutes ago   Up 12 minutes   0.0.0.0:8000->8000/tcp, 0.0.0.0:9000->9000/tcp   patroni-backend-1
ba9abcb9a31e   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-etcd4
9449bce0d164   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-patroni2
27929b25e40f   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes   0.0.0.0:5001->5000/tcp, 0.0.0.0:5002->5001/tcp   demo-haproxy
5a3da3d5ecaf   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-etcd1
5a3b385a8da2   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-patroni3
84119b30c3cb   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-etcd2
a88a3a1e847d   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 4 minutes                                                     demo-patroni6
073b657af490   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-patroni1
43e3b01069e8   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-patroni4
de932e4c5f98   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 4 minutes                                                     demo-patroni5
73e0a7edde6b   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-etcd3
8b48743a1ef6   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-etcd6
b274914d31ec   patroni           "/bin/sh /entrypoint…"   13 minutes ago   Up 12 minutes                                                    demo-etcd5

.patroni % docker logs de932e4c5f98
...
2024-05-05 11:43:14,091 INFO: no action. I am (patroni5), a secondary, and following a leader (patroni6)
```

Видим, что мастер patroni6 .

Тогда останавливаем например patroni1

Останавливаем Jmeter

Далее мы должны сделать promotion одной из реплик до мастера. В кластере patroni для этого достаточно просто остановить мастер и patroni сам подключит одну из реплик как мастер. Останавливаем мастер patroni6 например через docker desktop. Получаем:

```bash
.patroni % docker ps                        
CONTAINER ID   IMAGE             COMMAND                  CREATED          STATUS          PORTS                                            NAMES
cc22770d009f   patroni-backend   "cargo run --offline"    40 minutes ago   Up 39 minutes   0.0.0.0:8000->8000/tcp, 0.0.0.0:9000->9000/tcp   patroni-backend-1
ba9abcb9a31e   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-etcd4
9449bce0d164   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-patroni2
27929b25e40f   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes   0.0.0.0:5001->5000/tcp, 0.0.0.0:5002->5001/tcp   demo-haproxy
5a3da3d5ecaf   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-etcd1
5a3b385a8da2   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-patroni3
84119b30c3cb   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-etcd2
a88a3a1e847d   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 31 minutes                                                    demo-patroni6
43e3b01069e8   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 4 seconds                                                     demo-patroni4
de932e4c5f98   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 31 minutes                                                    demo-patroni5
73e0a7edde6b   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-etcd3
b274914d31ec   patroni           "/bin/sh /entrypoint…"   40 minutes ago   Up 40 minutes                                                    demo-etcd5

.patroni % docker logs de932e4c5f98
...
2024-05-05 12:21:14,998 INFO: no action. I am (patroni5), a secondary, and following a leader (patroni3)
```

Чтобы посмотреть количество записей, которое успел получить новый мастер (бывшая синхронная реплика) заходим в контейнер и выполняем sql запрос:
```bash
.patroni % docker exec -it de932e4c5f98 psql
psql (16.2 (Debian 16.2-1.pgdg120+2))
Type "help" for help.

postgres=# SELECT COUNT(id) FROM users;
 count 
-------
    47
(1 row)

postgres=# 
```

Получается бывшая реплика успела получить 47 записей.

Смотрим в отчет Jmeter и видим что он так же отправил 47 запросов.

![](./assets/jmeter_report/Screenshot%202024-05-05%20at%2015.25.34.png)
![](./assets/jmeter_report/Screenshot%202024-05-05%20at%2015.25.34%20(2).png)

Получается потерь данных небыло.