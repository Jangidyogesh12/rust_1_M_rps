-- Add up migration script here
create table if not exists messages (
  id SERIAL PRIMARY KEY,
  "from" TEXT NOT NULL,
  "to" TEXT NOT NULL,
  message TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT now()
);
