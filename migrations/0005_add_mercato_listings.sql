-- Migration: 0005_add_mercato_listings

-- Table to store marketplace listings indexed from ATProto
CREATE TABLE IF NOT EXISTS listings (
    uri TEXT PRIMARY KEY,
    authorDid TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    role TEXT NOT NULL, -- 'maker' or 'taker'
    price TEXT,
    barterFor TEXT,
    lat REAL,
    lng REAL,
    fuzz INTEGER,
    city TEXT,
    createdAt DATETIME NOT NULL,
    indexedAt DATETIME NOT NULL,
    createdViaThisApp INTEGER DEFAULT 0, -- boolean 0/1
    seenOnJetstream INTEGER DEFAULT 0 -- boolean 0/1
);

-- Index for searching nearby items (could use geohash in future, but simple lat/lng index for now)
CREATE INDEX IF NOT EXISTS idx_listings_author ON listings(authorDid);
CREATE INDEX IF NOT EXISTS idx_listings_created ON listings(createdAt DESC);
