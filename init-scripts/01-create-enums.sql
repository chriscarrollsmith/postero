-- Create enum types for Zotero sync
CREATE TYPE public.syncdirection AS ENUM (
    'none',
    'tocloud',
    'tolocal',
    'bothcloud',
    'bothlocal',
    'bothmanual'
);

CREATE TYPE public.syncstatus AS ENUM (
    'new',
    'synced',
    'modified',
    'incomplete'
); 