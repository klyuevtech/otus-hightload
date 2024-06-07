# Домашнее задание

## Применение tarantool'а как хранилища

В in-memory СУБД было решено хранить сессии пользователей. Т.к. сессии это обычно временная информация, то сессии безопасно хранить в такой СУБД даже без персистивного хранилища.

Работа с клиентской rust-библиотекой 'rusty_tarantool' реализована в tarantool.rs и tarantool_friends_storage.rs файлах.
Инициализация хранилища модуля session.rs происходит в main.rs в функции main():

```rust
    session::init_storage(Box::new(
        // postgres_session_storage::PostgresSessionStorage::new(
        //     postgres::get_master_pool_ref(),
        //     postgres::get_replica_pool_ref()
        // )
        TarantoolSessionStorage::new(tarantool::TarantoolClientManager::new().await)
    )).await;
```

Таким образом модуль session.rs получает и сохраняет данные через абстрактное хранилище.

Само же хранилище tarantool_session_storage.rs использует вызовы процедур lua для выполнения запросов к tarantool серверу:

```rust
#[async_trait]
impl SessionStorage for TarantoolSessionStorage {
    async fn create(&self, session: &Session) -> Result<Uuid, io::Error> {
        let session_tuple: (String, String, String) = self.get_leader_client().await
            .prepare_fn_call("session_create")
            .bind(session.get_id().to_string().as_str()).unwrap()
            .bind(session.get_user_id().to_string().as_str()).unwrap()
            .bind(session.get_data().to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_single().unwrap();

        Ok(uuid::Uuid::parse_str(session_tuple.0.as_str()).unwrap())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Session>, io::Error> {
        if let Ok(session_tuple) = self.get_client()
            .prepare_fn_call("session_get_by_id")
            .bind(id).unwrap()
            .execute().await.unwrap()
            .decode_single::<(String, String, String)>() {
                Ok(Some(
                    Session::new(
                        String::from(session_tuple.0),
                        String::from(session_tuple.1), 
                        String::from(session_tuple.2),
                    )
                ))
            } else {
                Ok(None)
            }
    }
}
```

Имя вызываемой процедуры передается в вызов функции 'prepare_fn_call' библиотеки "rusty_tarantool".

Выбор конкретного клиента для выполнения запроса происходит случайным образом из пула клиентов функцией `get_client()` реализации структуры `TarantoolClientManager`:

```rust
    pub fn get_client(&self) -> &Client {
        let range_end = self.clients.len()-1;
        if range_end == 0 {
            return &self.clients[0];
        }
        &self.clients[rand::thread_rng().gen_range(0..range_end)]
    }
```

Для определения текущего лидера в системе master-replica используется вызов lua процедуры `get_leader_name` хранимой в файле `.docker/tarantool/app.lua` в этом коде модуля `tarantool.rs`:

```rust
    pub async fn get_leader_client(&self) -> &Client {
        &self.clients[self.get_leader_index().await as usize]
    }

    async fn get_leader_index(&self) -> usize {
        let leader_name: String = self.get_client().prepare_fn_call("get_leader_name").execute().await.unwrap().decode_single().unwrap();
        let leader_index = leader_name.chars().last().unwrap().to_digit(10).unwrap() as usize - 1;
        leader_index
    }
```

## Сравнение производительности сессий хранимых Postgres и Tarantool

Чтобы нагрузить базу в tarantool так же были перенесены friends . Для заполнения случайными данными таблицы friends Postgres и Tarantool баз нужно раскомментировать код между этими строками в rostgres.rs:

```rust
// INSERT DUMMY FRIENDS DATA: BEGIN
...
// INSERT DUMMY FRIENDS DATA: BEGIN
```

и перезапустить контейнеры командой:

```bash
docker compose -f ./docker-compose.yaml up -d --force-recreate
```

после заполнения таблиц данными нужно закомментировать строки и снова перезапустить контейнер.


## Конфигурация Tarantool сервера

Для запуска сервера используем Docker образ tarantool:latest .
Верхнии слои образа описаны в файле:
.docker/tarantool/Dockerfile

Сам Tarantool сервер дополнительно конфигурируется этими тремя файлами:
.docker/tarantool/app.lua
.docker/tarantool/config.yaml
.docker/tarantool/instances.yml


### Результаты Postgres

![](./postgres/Screenshot%202024-06-06%20at%2011.33.20.png)
![](./postgres/Screenshot%202024-06-06%20at%2011.33.40.png)
![](./postgres/Screenshot%202024-06-06%20at%2011.33.56.png)
![](./postgres/Screenshot%202024-06-06%20at%2011.34.07.png)
![](./postgres/Screenshot%202024-06-06%20at%2011.34.42.png)

### Результаты Tarantool

![](./tarantool/Screenshot%202024-06-07%20at%2014.40.35.png)
![](./tarantool/Screenshot%202024-06-07%20at%2014.41.15.png)


По графикам видно, что ускорение работы при использовании Tarantool произошло примерно на порядок.