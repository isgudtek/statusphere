-- Migration: 0008_add_listing_images

ALTER TABLE listings ADD COLUMN imageCid TEXT;
