-- Create main tables for Zotero sync

-- Groups table
CREATE TABLE IF NOT EXISTS public.groups (
    id bigint PRIMARY KEY,
    version bigint DEFAULT 0 NOT NULL,
    created timestamp with time zone DEFAULT NOW(),
    modified timestamp with time zone DEFAULT NOW(),
    data jsonb,
    deleted boolean DEFAULT false NOT NULL,
    itemversion bigint DEFAULT 0,
    collectionversion bigint DEFAULT 0,
    tagversion bigint DEFAULT 0,
    gitlab timestamp with time zone
);

-- Items table
CREATE TABLE IF NOT EXISTS public.items (
    key varchar(8) NOT NULL,
    version bigint DEFAULT 0 NOT NULL,
    library bigint NOT NULL,
    sync public.syncstatus DEFAULT 'new' NOT NULL,
    data jsonb,
    meta jsonb,
    trashed boolean DEFAULT false NOT NULL,
    deleted boolean DEFAULT false NOT NULL,
    md5 varchar(32),
    modified timestamp with time zone DEFAULT NOW(),
    gitlab timestamp with time zone,
    PRIMARY KEY (key, library),
    FOREIGN KEY (library) REFERENCES public.groups(id)
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
    library bigint NOT NULL,
    sync public.syncstatus DEFAULT 'new' NOT NULL,
    data jsonb,
    meta jsonb,
    deleted boolean DEFAULT false NOT NULL,
    modified timestamp with time zone DEFAULT NOW(),
    gitlab timestamp with time zone,
    PRIMARY KEY (key, library),
    FOREIGN KEY (library) REFERENCES public.groups(id)
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
    library bigint NOT NULL,
    PRIMARY KEY (tag, library),
    FOREIGN KEY (library) REFERENCES public.groups(id)
);

-- Syncgroups table (control table)
CREATE TABLE IF NOT EXISTS public.syncgroups (
    id bigint PRIMARY KEY,
    active boolean DEFAULT true NOT NULL,
    direction public.syncdirection DEFAULT 'none' NOT NULL,
    tags boolean DEFAULT false NOT NULL,
    FOREIGN KEY (id) REFERENCES public.groups(id)
);

-- Add direction column to syncgroups if it doesn't exist (in case it was dropped by enum CASCADE)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'syncgroups' AND column_name = 'direction' AND table_schema = 'public') THEN
        ALTER TABLE public.syncgroups ADD COLUMN direction public.syncdirection DEFAULT 'none' NOT NULL;
    END IF;
END$$;

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_items_library ON public.items(library);
CREATE INDEX IF NOT EXISTS idx_items_sync ON public.items(sync);
CREATE INDEX IF NOT EXISTS idx_items_deleted ON public.items(deleted);

CREATE INDEX IF NOT EXISTS idx_collections_library ON public.collections(library);
CREATE INDEX IF NOT EXISTS idx_collections_sync ON public.collections(sync);
CREATE INDEX IF NOT EXISTS idx_collections_deleted ON public.collections(deleted);
CREATE INDEX IF NOT EXISTS idx_tags_library ON public.tags(library);



-- Create constraint name referenced in Go code for tags
-- Drop the constraint first if it exists, then add it.
-- This assumes the constraint name is unique enough not to cause issues if dropped.
ALTER TABLE public.tags DROP CONSTRAINT IF EXISTS pk_tags;
ALTER TABLE public.tags ADD CONSTRAINT pk_tags UNIQUE (tag, library);

-- Consider adding these for better query performance:
CREATE INDEX IF NOT EXISTS idx_items_data_itemtype ON public.items USING GIN ((data->>'itemType') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_items_data_title ON public.items USING GIN ((data->>'title') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_collections_data_name ON public.collections USING GIN ((data->>'name') gin_trgm_ops); 