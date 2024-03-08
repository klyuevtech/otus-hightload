CREATE TABLE users (
  id UUID DEFAULT uuid_generate_v4(),
  first_name VARCHAR(32),
  second_name VARCHAR(32),
  birthdate VARCHAR(10),
  biography VARCHAR(2048),
  city VARCHAR(32),
  password_hash VARCHAR(128),
  salt VARCHAR(32)
);
