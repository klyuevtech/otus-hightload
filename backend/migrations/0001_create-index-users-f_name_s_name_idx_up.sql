CREATE INDEX IF NOT EXISTS f_name_s_name_idx ON users USING gin (first_name gin_trgm_ops, second_name gin_trgm_ops);