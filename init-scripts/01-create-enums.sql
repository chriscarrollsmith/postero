-- Create enum types for Zotero sync

-- Create library_type enum if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'library_type') THEN
        CREATE TYPE public.library_type AS ENUM (
            'user',
            'group'
        );
    END IF;
END$$;

-- Create syncdirection enum if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'syncdirection') THEN
        CREATE TYPE public.syncdirection AS ENUM (
            'none',
            'tocloud',
            'tolocal',
            'bothcloud',
            'bothlocal',
            'bothmanual'
        );
    END IF;
END$$;

-- Create syncstatus enum if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'syncstatus') THEN
        CREATE TYPE public.syncstatus AS ENUM (
            'new',
            'synced',
            'modified',
            'incomplete'
        );
    END IF;
END$$; 