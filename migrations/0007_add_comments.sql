-- Migration: 0007_add_comments

CREATE TABLE IF NOT EXISTS comments (
    uri TEXT PRIMARY KEY,
    authorDid TEXT NOT NULL,
    subjectUri TEXT NOT NULL,
    content TEXT NOT NULL,
    createdAt TEXT NOT NULL,
    indexedAt TEXT NOT NULL,
    seenOnJetstream BOOLEAN NOT NULL,
    createdViaThisApp BOOLEAN NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_comments_subject ON comments (subjectUri);
