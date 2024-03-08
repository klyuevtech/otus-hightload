CREATE TABLE sessions (
  id UUID DEFAULT uuid_generate_v4(),
  user_id UUID,
  data TEXT,
  time_created TIMESTAMP NOT NULL DEFAULT NOW(),
  time_updated TIMESTAMP NOT NULL DEFAULT NOW()
);
