-- Create main tables for Zotero sync

-- Libraries table
CREATE TABLE IF NOT EXISTS public.libraries (
    id bigint NOT NULL,
    library_type public.library_type NOT NULL,
    version bigint DEFAULT 0 NOT NULL,
    created timestamp with time zone DEFAULT NOW(),
    modified timestamp with time zone DEFAULT NOW(),
    data jsonb,
    deleted boolean DEFAULT false NOT NULL,
    item_version bigint DEFAULT 0,
    collection_version bigint DEFAULT 0,
    tag_version bigint DEFAULT 0,
    gitlab timestamp with time zone,
    PRIMARY KEY (id, library_type)
);

-- Items table
CREATE TABLE IF NOT EXISTS public.items (
    key varchar(8) NOT NULL,
    version bigint DEFAULT 0 NOT NULL,
    library_id bigint NOT NULL,
    library_type public.library_type NOT NULL,
    sync public.syncstatus DEFAULT 'new' NOT NULL,
    data jsonb,
    meta jsonb,
    trashed boolean DEFAULT false NOT NULL,
    deleted boolean DEFAULT false NOT NULL,
    md5 varchar(32),
    modified timestamp with time zone DEFAULT NOW(),
    gitlab timestamp with time zone,
    PRIMARY KEY (key, library_id, library_type),
    FOREIGN KEY (library_id, library_type) REFERENCES public.libraries(id, library_type)
);

-- Add sync column to items if it doesn't exist (in case it was dropped by enum CASCADE)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'items' AND column_name = 'sync' AND table_schema = 'public') THEN
        ALTER TABLE public.items ADD COLUMN sync public.syncstatus DEFAULT 'new' NOT NULL;
    END IF;
END$$;

-- Collections table
CREATE TABLE IF NOT EXISTS public.collections (
    key varchar(8) NOT NULL,
    version bigint DEFAULT 0 NOT NULL,
    library_id bigint NOT NULL,
    library_type public.library_type NOT NULL,
    sync public.syncstatus DEFAULT 'new' NOT NULL,
    data jsonb,
    meta jsonb,
    deleted boolean DEFAULT false NOT NULL,
    modified timestamp with time zone DEFAULT NOW(),
    gitlab timestamp with time zone,
    PRIMARY KEY (key, library_id, library_type),
    FOREIGN KEY (library_id, library_type) REFERENCES public.libraries(id, library_type)
);

-- Add sync column to collections if it doesn't exist (in case it was dropped by enum CASCADE)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'collections' AND column_name = 'sync' AND table_schema = 'public') THEN
        ALTER TABLE public.collections ADD COLUMN sync public.syncstatus DEFAULT 'new' NOT NULL;
    END IF;
END$$;

-- Tags table
CREATE TABLE IF NOT EXISTS public.tags (
    tag varchar(255) NOT NULL,
    meta jsonb,
    library_id bigint NOT NULL,
    library_type public.library_type NOT NULL,
    PRIMARY KEY (tag, library_id, library_type),
    FOREIGN KEY (library_id, library_type) REFERENCES public.libraries(id, library_type)
);

-- Sync libraries table
CREATE TABLE IF NOT EXISTS public.sync_libraries (
    library_id bigint NOT NULL,
    library_type public.library_type NOT NULL,
    active boolean DEFAULT true NOT NULL,
    direction public.syncdirection DEFAULT 'none' NOT NULL,  -- Deprecated: use incoming_sync/outgoing_sync
    incoming_sync public.syncmode DEFAULT 'disabled' NOT NULL,
    outgoing_sync public.syncmode DEFAULT 'disabled' NOT NULL,
    tags boolean DEFAULT false NOT NULL,
    PRIMARY KEY (library_id, library_type),
    FOREIGN KEY (library_id, library_type) REFERENCES public.libraries(id, library_type) ON DELETE CASCADE
);

-- Add direction column to sync_libraries if it doesn't exist (in case it was dropped by enum CASCADE)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'sync_libraries' AND column_name = 'direction' AND table_schema = 'public') THEN
        ALTER TABLE public.sync_libraries ADD COLUMN direction public.syncdirection DEFAULT 'none' NOT NULL;
    END IF;
END$$;

-- Add incoming_sync column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'sync_libraries' AND column_name = 'incoming_sync' AND table_schema = 'public') THEN
        ALTER TABLE public.sync_libraries ADD COLUMN incoming_sync public.syncmode DEFAULT 'disabled' NOT NULL;
    END IF;
END$$;

-- Add outgoing_sync column if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'sync_libraries' AND column_name = 'outgoing_sync' AND table_schema = 'public') THEN
        ALTER TABLE public.sync_libraries ADD COLUMN outgoing_sync public.syncmode DEFAULT 'disabled' NOT NULL;
    END IF;
END$$;

-- Sync queue for event-driven outgoing sync
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

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_items_library ON public.items(library_id, library_type);
CREATE INDEX IF NOT EXISTS idx_items_sync ON public.items(sync);
CREATE INDEX IF NOT EXISTS idx_items_deleted ON public.items(deleted);

CREATE INDEX IF NOT EXISTS idx_collections_library ON public.collections(library_id, library_type);
CREATE INDEX IF NOT EXISTS idx_collections_sync ON public.collections(sync);
CREATE INDEX IF NOT EXISTS idx_collections_deleted ON public.collections(deleted);
CREATE INDEX IF NOT EXISTS idx_tags_library ON public.tags(library_id, library_type);

-- Create constraint name referenced in Go code for tags
-- Drop the constraint first if it exists, then add it.
-- This assumes the constraint name is unique enough not to cause issues if dropped.
ALTER TABLE public.tags DROP CONSTRAINT IF EXISTS pk_tags;
ALTER TABLE public.tags ADD CONSTRAINT pk_tags UNIQUE (tag, library_id, library_type);

-- Consider adding these for better query performance:
CREATE INDEX IF NOT EXISTS idx_items_data_itemtype ON public.items USING GIN ((data->>'itemType') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_items_data_title ON public.items USING GIN ((data->>'title') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_collections_data_name ON public.collections USING GIN ((data->>'name') gin_trgm_ops);

-- Sync queue indexes
CREATE INDEX IF NOT EXISTS idx_sync_queue_pending
ON public.sync_queue (library_id, library_type, next_retry_at)
WHERE processed_at IS NULL AND retry_count < max_retries;

CREATE INDEX IF NOT EXISTS idx_sync_queue_processed
ON public.sync_queue (processed_at)
WHERE processed_at IS NOT NULL;

-- Unique constraint for pending sync entries (prevents duplicates)
-- Using DO block to handle case where constraint already exists
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

-- Trigger function to enqueue sync operations for event-driven sync
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

-- Trigger for items table
DROP TRIGGER IF EXISTS items_sync_queue_trigger ON public.items;
CREATE TRIGGER items_sync_queue_trigger
    AFTER INSERT OR UPDATE OR DELETE ON public.items
    FOR EACH ROW EXECUTE FUNCTION public.enqueue_sync('item');

-- Trigger for collections table
DROP TRIGGER IF EXISTS collections_sync_queue_trigger ON public.collections;
CREATE TRIGGER collections_sync_queue_trigger
    AFTER INSERT OR UPDATE OR DELETE ON public.collections
    FOR EACH ROW EXECUTE FUNCTION public.enqueue_sync('collection'); 