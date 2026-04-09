-- Migration: 0006_update_listings_geo

ALTER TABLE listings ADD COLUMN locationName TEXT;
ALTER TABLE listings ADD COLUMN altitude TEXT;
ALTER TABLE listings ADD COLUMN latitude TEXT; -- Store as text per user lexicon
ALTER TABLE listings ADD COLUMN longitude TEXT; -- Store as text per user lexicon
