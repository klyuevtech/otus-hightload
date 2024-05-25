Для храниния ленты постов испольуем Redis в режиме кеша.

Лента формируется в методе "get_feed" реализации структуры Post:
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/post.rs#L120

Алгоритм работы такой:
1. Формируем ключ кэша, проверяем доступность кэша (есть ли вообще подключение к Redis) и наличие данных по ключу в кэше.
2. Если кэш недоступен или данные по ключу отсутствуют, то формируем ленту запросом к базе данных. Если кэш доступен (есть подключение к Redis) то записываем данные по ключу в кэш. Возвращаем результат из функции.
3. Если на шаге 1 данные уже были в кэше, то минуя шаг 2 возвращаем даннее из кэша по сформированному ключу.

Для кэша было решено использовать Redis List:
https://redis.io/docs/latest/develop/data-types/lists/

Уданной структуры есть метод LTRIM:
https://redis.io/docs/latest/commands/ltrim/
который позволяет урезать список до определенного количества элементов. Таким образом можно будет удобно ограничивать количество постов в ленте до 1000.

Так как List в Redis - это связанный список, то также будет удобно добавлять свежий пост в начало ленты.

Для обновления/инвалидации данных кэша испольуется RabbitMQ очередь. Присоздании поста происходит добавление его в список кэша ленты пользователя. При обновлении и удалении поста происходит просто инвалидация кэша - удаление списка по ключу из кэша. Публикация события происходит вызовом метода "self::publish_message".
Метод Post::create:
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/post.rs#L279

Методы Post::update и Post::delete:
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/post.rs#L291
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/post.rs#L315

Если нужно сделать так, чтобы в ленте отображался только по одному посту от каждого друга, то нужно присвоить true переменной окружения POSTS_FEED_ONE_POST_PER_USER.

Провайдеры Redis и RabbitMQ реализованы в этих файлах:
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/redis.rs
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/rabbitmq.rs

Для проверки работоспособности нужно склонировать себе проект из репозитория и поднять контейнеры используя например docker compose конфигурацию:
```bash
git clone git@github.com:klyuevtech/otus-hightload.git
cd .docker
docker compose -f ./docker-compose.yaml up -d --force-recreate
```