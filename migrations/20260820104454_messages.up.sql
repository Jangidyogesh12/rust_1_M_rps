-- Add up migration script here
create table if not exists messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "from" TEXT NOT NULL,
  "to" TEXT NOT NULL,
  message TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT now()
);
