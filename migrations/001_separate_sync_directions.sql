-- Migration: Separate incoming and outgoing sync directions
-- This migration adds the new syncmode enum and incoming_sync/outgoing_sync columns,
-- then migrates data from the deprecated direction column.

-- Step 1: Create syncmode enum if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'syncmode') THEN
        CREATE TYPE public.syncmode AS ENUM ('disabled', 'manual');
    END IF;
END$$;

-- Step 2: Add incoming_sync column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'sync_libraries' AND column_name = 'incoming_sync' AND table_schema = 'public') THEN
        ALTER TABLE public.sync_libraries ADD COLUMN incoming_sync public.syncmode DEFAULT 'disabled' NOT NULL;
    END IF;
END$$;

-- Step 3: Add outgoing_sync column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'sync_libraries' AND column_name = 'outgoing_sync' AND table_schema = 'public') THEN
        ALTER TABLE public.sync_libraries ADD COLUMN outgoing_sync public.syncmode DEFAULT 'disabled' NOT NULL;
    END IF;
END$$;

-- Step 4: Migrate existing direction values to new columns
-- Only update rows where the new columns are still at default (disabled)
-- This ensures we don't overwrite any values set after the columns were added
UPDATE public.sync_libraries SET
    incoming_sync = CASE
        WHEN direction IN ('tolocal', 'bothcloud', 'bothlocal', 'bothmanual') THEN 'manual'::syncmode
        ELSE 'disabled'::syncmode
    END,
    outgoing_sync = CASE
        WHEN direction IN ('tocloud', 'bothcloud', 'bothlocal', 'bothmanual') THEN 'manual'::syncmode
        ELSE 'disabled'::syncmode
    END
WHERE incoming_sync = 'disabled' AND outgoing_sync = 'disabled' AND direction != 'none';

-- Note: The old 'direction' column is kept for backwards compatibility
-- It can be dropped in a future migration once all clients are updated
