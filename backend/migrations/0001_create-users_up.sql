CREATE TABLE IF NOT EXISTS users (
  id UUID DEFAULT uuid_generate_v4(),
  first_name VARCHAR(64),
  second_name VARCHAR(64),
  textsearchable_first_name tsvector GENERATED ALWAYS AS (to_tsvector('russian', coalesce(first_name, ''))) STORED,
  textsearchable_second_name tsvector GENERATED ALWAYS AS (to_tsvector('russian', coalesce(second_name, ''))) STORED,
  textsearchable_names tsvector GENERATED ALWAYS AS (to_tsvector('russian', coalesce(first_name, '') || ' ' || coalesce(second_name, '') )) STORED,
  birthdate DATE,
  biography VARCHAR(2048),
  city VARCHAR(128),
  password_hash VARCHAR(128),
  salt VARCHAR(32)
);
