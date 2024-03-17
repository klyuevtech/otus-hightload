CREATE TABLE IF NOT EXISTS users (
  id UUID DEFAULT uuid_generate_v4(),
  first_name VARCHAR(64),
  second_name VARCHAR(64),
  birthdate DATE,
  biography VARCHAR(2048),
  city VARCHAR(128),
  password_hash VARCHAR(128),
  salt VARCHAR(32)
);
