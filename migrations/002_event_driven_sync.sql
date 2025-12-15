-- Migration: Event-Driven Outgoing Sync
-- Adds 'event_driven' to syncmode enum, creates sync_queue table, and triggers

-- Step 1: Add 'event_driven' value to syncmode enum if not exists
DO $$
BEGIN
    -- Check if the value already exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_enum e
        JOIN pg_type t ON e.enumtypid = t.oid
        WHERE t.typname = 'syncmode' AND e.enumlabel = 'event_driven'
    ) THEN
        ALTER TYPE public.syncmode ADD VALUE 'event_driven';
    END IF;
END$$;

-- Step 2: Create sync_queue table if not exists
CREATE TABLE IF NOT EXISTS public.sync_queue (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(20) NOT NULL,  -- 'item', 'collection'
    entity_key VARCHAR(8) NOT NULL,
    library_id BIGINT NOT NULL,
    library_type public.library_type NOT NULL,
    operation VARCHAR(10) NOT NULL,    -- 'create', 'update', 'delete'
    priority INT DEFAULT 0,
    retry_count INT DEFAULT 0,
    max_retries INT DEFAULT 5,
    next_retry_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_error TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE,
    FOREIGN KEY (library_id, library_type) REFERENCES public.libraries(id, library_type) ON DELETE CASCADE
);

-- Step 3: Create indexes for sync_queue
CREATE INDEX IF NOT EXISTS idx_sync_queue_pending
ON public.sync_queue (library_id, library_type, next_retry_at)
WHERE processed_at IS NULL AND retry_count < max_retries;

CREATE INDEX IF NOT EXISTS idx_sync_queue_processed
ON public.sync_queue (processed_at)
WHERE processed_at IS NOT NULL;

-- Step 4: Add unique constraint if not exists
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'unique_pending_sync'
    ) THEN
        ALTER TABLE public.sync_queue
        ADD CONSTRAINT unique_pending_sync
        UNIQUE (entity_type, entity_key, library_id, library_type, operation);
    END IF;
END$$;

-- Step 5: Create or replace trigger function
CREATE OR REPLACE FUNCTION public.enqueue_sync()
RETURNS TRIGGER AS $$
DECLARE
    v_outgoing_sync public.syncmode;
    v_library_id BIGINT;
    v_library_type public.library_type;
    v_entity_key VARCHAR(8);
BEGIN
    -- Determine library info from NEW or OLD record
    IF TG_OP = 'DELETE' THEN
        v_library_id := OLD.library_id;
        v_library_type := OLD.library_type;
        v_entity_key := OLD.key;
    ELSE
        v_library_id := NEW.library_id;
        v_library_type := NEW.library_type;
        v_entity_key := NEW.key;
    END IF;

    -- Check if event-driven sync is enabled for this library
    SELECT outgoing_sync INTO v_outgoing_sync
    FROM public.sync_libraries
    WHERE library_id = v_library_id
      AND library_type = v_library_type;

    -- Only enqueue if event_driven mode is enabled
    IF v_outgoing_sync IS NULL OR v_outgoing_sync != 'event_driven' THEN
        RETURN COALESCE(NEW, OLD);
    END IF;

    -- Enqueue based on operation type
    IF TG_OP = 'DELETE' THEN
        INSERT INTO public.sync_queue (entity_type, entity_key, library_id, library_type, operation)
        VALUES (TG_ARGV[0], v_entity_key, v_library_id, v_library_type, 'delete')
        ON CONFLICT (entity_type, entity_key, library_id, library_type, operation)
        DO UPDATE SET next_retry_at = NOW(), retry_count = 0, processed_at = NULL;

    ELSIF TG_OP = 'INSERT' THEN
        INSERT INTO public.sync_queue (entity_type, entity_key, library_id, library_type, operation)
        VALUES (TG_ARGV[0], v_entity_key, v_library_id, v_library_type, 'create')
        ON CONFLICT (entity_type, entity_key, library_id, library_type, operation)
        DO UPDATE SET next_retry_at = NOW(), retry_count = 0, processed_at = NULL;

    ELSIF TG_OP = 'UPDATE' THEN
        -- Only enqueue if meaningful data changed (not just sync status)
        IF NEW.data IS DISTINCT FROM OLD.data OR NEW.deleted != OLD.deleted THEN
            INSERT INTO public.sync_queue (entity_type, entity_key, library_id, library_type, operation)
            VALUES (TG_ARGV[0], v_entity_key, v_library_id, v_library_type, 'update')
            ON CONFLICT (entity_type, entity_key, library_id, library_type, operation)
            DO UPDATE SET next_retry_at = NOW(), retry_count = 0, processed_at = NULL;
        END IF;
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Step 6: Create triggers (drop first to handle re-running migration)
DROP TRIGGER IF EXISTS items_sync_queue_trigger ON public.items;
CREATE TRIGGER items_sync_queue_trigger
    AFTER INSERT OR UPDATE OR DELETE ON public.items
    FOR EACH ROW EXECUTE FUNCTION public.enqueue_sync('item');

DROP TRIGGER IF EXISTS collections_sync_queue_trigger ON public.collections;
CREATE TRIGGER collections_sync_queue_trigger
    AFTER INSERT OR UPDATE OR DELETE ON public.collections
    FOR EACH ROW EXECUTE FUNCTION public.enqueue_sync('collection');
