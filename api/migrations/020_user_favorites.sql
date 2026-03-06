CREATE TABLE user_favorites (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  database_id UUID NOT NULL REFERENCES database_instances(id) ON DELETE CASCADE,
  PRIMARY KEY (user_id, database_id)
)