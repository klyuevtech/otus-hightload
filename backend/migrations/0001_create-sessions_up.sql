CREATE TABLE IF NOT EXISTS sessions (
  id UUID DEFAULT uuid_generate_v4(),
  user_id UUID,
  data JSONB NOT NULL DEFAULT '{}'::jsonb,
  time_created TIMESTAMP NOT NULL DEFAULT NOW(),
  time_updated TIMESTAMP NOT NULL DEFAULT NOW()
);
