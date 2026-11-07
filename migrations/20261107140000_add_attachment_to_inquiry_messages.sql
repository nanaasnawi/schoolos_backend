-- Add attachment columns to inquiry_messages for chat photo/file sharing
ALTER TABLE inquiry_messages 
ADD COLUMN IF NOT EXISTS attachment_url TEXT,
ADD COLUMN IF NOT EXISTS attachment_type VARCHAR(50);

