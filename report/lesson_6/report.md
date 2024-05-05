**Структура базы**

```sql
postgres=# \d users;
                                                                                       Table "public.users"
           Column           |          Type           | Collation | Nullable |                                                      Default                                                       
----------------------------+-------------------------+-----------+----------+--------------------------------------------------------------------------------------------------------------------
 id                         | uuid                    |           |          | uuid_generate_v4()
 first_name                 | character varying(64)   |           |          | 
 second_name                | character varying(64)   |           |          | 
 birthdate                  | date                    |           |          | 
 biography                  | character varying(2048) |           |          | 
 city                       | character varying(128)  |           |          | 
 password_hash              | character varying(128)  |           |          | 
 salt                       | character varying(32)   |           |          | 
 textsearchable_first_name  | tsvector                |           |          | generated always as (to_tsvector('russian'::regconfig, COALESCE(first_name, ''::character varying)::text)) stored
 textsearchable_second_name | tsvector                |           |          | generated always as (to_tsvector('russian'::regconfig, COALESCE(second_name, ''::character varying)::text)) stored
Indexes:
    "id_idx" UNIQUE, btree (id)
    "users_names_gin_trgm_idx" gin (first_name gin_trgm_ops, second_name gin_trgm_ops)
    "users_names_gin_tsvector_1_idx" gin (textsearchable_first_name, textsearchable_second_name)

postgres=# select count(id) from users;
  count  
---------
 1999864
(1 row)

postgres=# select * from users limit 1;
                  id                  | first_name | second_name | birthdate  |                                                          biography                                                          |   city   | password_hash  | salt | textsearchable_first_name | textsearchable_second_name 
--------------------------------------+------------+-------------+------------+-----------------------------------------------------------------------------------------------------------------------------+----------+----------------+------+---------------------------+----------------------------
 281676e9-9239-45f0-bc47-ef8b4f77a4b3 | Роберт     | Абрамов     | 2012-01-01 | Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. | Воткинск | mydummypass123 | salt | 'роберт':1                | 'абрам':1
(1 row)
```

Индексы были добавлены уже после того как были сняты результаты тестов без индекса.

---

Зарпос на поиск в коде такой:

```sql
SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE textsearchable_first_name @@ to_tsquery('russian', $1) AND textsearchable_second_name @@ to_tsquery('russian', $2) ORDER BY id
```

**Результаты тестов без индекса**

---

**Number of threads: 10**

**Rump-up period: 10**

**Loop count: 1**

![](./without_index/10/Снимок%20экрана%202024-03-30%20в%2009.45.49.png)
![](./without_index/10/Снимок%20экрана%202024-03-30%20в%2009.46.11.png)
![](./without_index/10/Снимок%20экрана%202024-03-30%20в%2009.46.14.png)
![](./without_index/10/Снимок%20экрана%202024-03-30%20в%2009.46.47.png)
![](./without_index/10/Снимок%20экрана%202024-03-30%20в%2009.46.49.png)

---

**Number of threads: 100**

**Rump-up period: 10**

**Loop count: 1**

![](./without_index/100/Снимок%20экрана%202024-03-30%20в%2009.47.59.png)
![](./without_index/100/Снимок%20экрана%202024-03-30%20в%2009.48.16.png)
![](./without_index/100/Снимок%20экрана%202024-03-30%20в%2009.48.29%201.png)
![](./without_index/100/Снимок%20экрана%202024-03-30%20в%2009.48.29.png)

---

**Number of threads: 1000**

**Rump-up period: 10**

**Loop count: 1**

![](./without_index/1000/Снимок%20экрана%202024-03-30%20в%2010.21.40.png)
![](./without_index/1000/Снимок%20экрана%202024-03-30%20в%2010.21.43.png)
![](./without_index/1000/Снимок%20экрана%202024-03-30%20в%2010.22.07.png)
![](./without_index/1000/Снимок%20экрана%202024-03-30%20в%2010.22.10.png)
![](./without_index/1000/Снимок%20экрана%202024-03-30%20в%2010.22.14.png)


**Результаты тестов после добавления индекса**

**Зарпос на добавление индекса**

```sql
CREATE UNIQUE INDEX IF NOT EXISTS id_idx ON users (id);
CREATE INDEX IF NOT EXISTS users_names_gin_tsvector_idx ON users USING GIN (textsearchable_first_name, textsearchable_second_name);
```


**Результат запроса EXPLAIN ANALYZE для запроса поиска**

```sql
postgres=# EXPLAIN ANALYZE SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE textsearchable_first_name @@ to_tsquery('russian', 'test:*') AND textsearchable_second_name @@ to_tsquery('russian', 'test:*') ORDER BY id
;
                                                                   QUERY PLAN                                                                    
-------------------------------------------------------------------------------------------------------------------------------------------------
 Sort  (cost=3896.18..3898.18 rows=800 width=191) (actual time=0.110..0.111 rows=1 loops=1)
   Sort Key: id
   Sort Method: quicksort  Memory: 25kB
   ->  Bitmap Heap Scan on users  (cost=888.20..3857.61 rows=800 width=191) (actual time=0.077..0.078 rows=1 loops=1)
         Recheck Cond: ((textsearchable_first_name @@ '''test'':*'::tsquery) AND (textsearchable_second_name @@ '''test'':*'::tsquery))
         Heap Blocks: exact=1
         ->  Bitmap Index Scan on users_names_gin_tsvector_1_idx  (cost=0.00..888.00 rows=800 width=0) (actual time=0.054..0.054 rows=1 loops=1)
               Index Cond: ((textsearchable_first_name @@ '''test'':*'::tsquery) AND (textsearchable_second_name @@ '''test'':*'::tsquery))
 Planning Time: 0.490 ms
 Execution Time: 0.289 ms
(10 rows)
```

Индекс `id_idx` может применять для сортировки по полю `id`. Тип поля `UUID`, поэтому выбрал тип индекса `BTREE` .

Индекс `users_names_gin_tsvector_idx` может применяться для выборки данных по полям `textsearchable_first_name` и `textsearchable_second_name`. По полям `first_name` и `second_name` нужен полнотекстовый поиск по префиксу. Поэтому выбираем тип индекса `GIN`, который работает с данными типа tsvector. Чтобы ускорить SELECT операции и не вызывать функцию to_tsvector на каждую строку, были созданы дополнительные поля `textsearchable_first_name` и `textsearchable_second_name` с предзаполненными значениями результатов вызова to_tsvector для каждого значения соответствующего поля first_name и second_name.

---

**Number of threads: 10**

**Rump-up period: 10**

**Loop count: 1**

![](./with_index/10/Снимок%20экрана%202024-03-30%20в%2010.27.41.png)
![](./with_index/10/Снимок%20экрана%202024-03-30%20в%2010.27.52.png)
![](./with_index/10/Снимок%20экрана%202024-03-30%20в%2010.28.42.png)
![](./with_index/10/Снимок%20экрана%202024-03-30%20в%2010.28.45.png)
![](./with_index/10/Снимок%20экрана%202024-03-30%20в%2010.28.47.png)

---

**Number of threads: 100**

**Rump-up period: 10**

**Loop count: 1**

![](./with_index/100/Снимок%20экрана%202024-03-30%20в%2010.29.59.png)
![](./with_index/100/Снимок%20экрана%202024-03-30%20в%2010.30.03.png)
![](./with_index/100/Снимок%20экрана%202024-03-30%20в%2010.30.13.png)
![](./with_index/100/Снимок%20экрана%202024-03-30%20в%2010.30.17.png)
![](./with_index/100/Снимок%20экрана%202024-03-30%20в%2010.30.19.png)

---

**Number of threads: 1000**

**Rump-up period: 10**

**Loop count: 1**

![](./with_index/1000/Снимок%20экрана%202024-03-30%20в%2010.31.24.png)
![](./with_index/1000/Снимок%20экрана%202024-03-30%20в%2010.31.29.png)
![](./with_index/1000/Снимок%20экрана%202024-03-30%20в%2010.31.33.png)
![](./with_index/1000/Снимок%20экрана%202024-03-30%20в%2010.31.35.png)
![](./with_index/1000/Снимок%20экрана%202024-03-30%20в%2010.31.37.png)

---

**Number of threads: 1000**

**Rump-up period: 10**

**Loop count: 100**

![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.46.47.png)
![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.46.53.png)
![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.47.23.png)
![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.47.25.png)
![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.47.28.png)
![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.47.42.png)
![](./with_index/1000-100/Снимок%20экрана%202024-03-30%20в%2010.47.55.png)

---

**Number of threads: 10**

**Rump-up period: 10**

**Loop count: 150**

![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.31.png)
![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.35.png)
![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.40.png)
![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.45.png)
![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.47.png)
![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.53.png)
![](./with_index/1000-150/Снимок%20экрана%202024-03-30%20в%2011.03.55.png)


**Зарпос на добавление индекса**

```sql
CREATE UNIQUE INDEX IF NOT EXISTS id_idx ON users (id);
CREATE INDEX IF NOT EXISTS users_names_gin_tsvector_idx ON users USING GIN (textsearchable_first_name, textsearchable_second_name);
```


**Результат запроса EXPLAIN ANALYZE для запроса поиска**

```sql
postgres=# EXPLAIN ANALYZE SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE textsearchable_first_name @@ to_tsquery('russian', 'test:*') AND textsearchable_second_name @@ to_tsquery('russian', 'test:*') ORDER BY id
;
                                                                   QUERY PLAN                                                                    
-------------------------------------------------------------------------------------------------------------------------------------------------
 Sort  (cost=3896.18..3898.18 rows=800 width=191) (actual time=0.110..0.111 rows=1 loops=1)
   Sort Key: id
   Sort Method: quicksort  Memory: 25kB
   ->  Bitmap Heap Scan on users  (cost=888.20..3857.61 rows=800 width=191) (actual time=0.077..0.078 rows=1 loops=1)
         Recheck Cond: ((textsearchable_first_name @@ '''test'':*'::tsquery) AND (textsearchable_second_name @@ '''test'':*'::tsquery))
         Heap Blocks: exact=1
         ->  Bitmap Index Scan on users_names_gin_tsvector_1_idx  (cost=0.00..888.00 rows=800 width=0) (actual time=0.054..0.054 rows=1 loops=1)
               Index Cond: ((textsearchable_first_name @@ '''test'':*'::tsquery) AND (textsearchable_second_name @@ '''test'':*'::tsquery))
 Planning Time: 0.490 ms
 Execution Time: 0.289 ms
(10 rows)
```

Индекс `id_idx` может применять для сортировки по полю `id`. Тип поля `UUID`, поэтому выбрал тип индекса `BTREE` .

Индекс `users_names_gin_tsvector_idx` может применяться для выборки данных по полям `textsearchable_first_name` и `textsearchable_second_name`. По полям `first_name` и `second_name` нужен полнотекстовый поиск по префиксу. Поэтому выбираем тип индекса `GIN`, который работает с данными типа tsvector. Чтобы ускорить SELECT операции и не вызывать функцию to_tsvector на каждую строку, были созданы дополнительные поля `textsearchable_first_name` и `textsearchable_second_name` с предзаполненными значениями результатов вызова to_tsvector для каждого значения соответствующего поля first_name и second_name.