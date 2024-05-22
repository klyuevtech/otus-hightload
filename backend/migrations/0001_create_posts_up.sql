CREATE TABLE IF NOT EXISTS posts (
  id UUID DEFAULT uuid_generate_v4(),
  content TEXT,
  user_id UUID,
  time_created TIMESTAMP NOT NULL DEFAULT NOW(),
  time_updated TIMESTAMP NOT NULL DEFAULT NOW()
);
