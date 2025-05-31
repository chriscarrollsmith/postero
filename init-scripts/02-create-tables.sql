-- Create main tables for Zotero sync

-- Groups table
CREATE TABLE public.groups (
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
CREATE TABLE public.items (
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

-- Collections table
CREATE TABLE public.collections (
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

-- Tags table
CREATE TABLE public.tags (
    tag varchar(255) NOT NULL,
    meta jsonb,
    library bigint NOT NULL,
    PRIMARY KEY (tag, library),
    FOREIGN KEY (library) REFERENCES public.groups(id)
);

-- Syncgroups table (control table)
CREATE TABLE public.syncgroups (
    id bigint PRIMARY KEY,
    active boolean DEFAULT true NOT NULL,
    direction public.syncdirection DEFAULT 'none' NOT NULL,
    tags boolean DEFAULT false NOT NULL,
    FOREIGN KEY (id) REFERENCES public.groups(id)
);

-- Create indexes for better performance
CREATE INDEX idx_items_library ON public.items(library);
CREATE INDEX idx_items_sync ON public.items(sync);
CREATE INDEX idx_items_deleted ON public.items(deleted);

CREATE INDEX idx_collections_library ON public.collections(library);
CREATE INDEX idx_collections_sync ON public.collections(sync);
CREATE INDEX idx_collections_deleted ON public.collections(deleted);
CREATE INDEX idx_tags_library ON public.tags(library);



-- Create constraint name referenced in Go code for tags
ALTER TABLE public.tags ADD CONSTRAINT pk_tags UNIQUE (tag, library);

-- Consider adding these for better query performance:
CREATE INDEX IF NOT EXISTS idx_items_data_itemtype ON public.items USING GIN ((data->>'itemType') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_items_data_title ON public.items USING GIN ((data->>'title') gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_collections_data_name ON public.collections USING GIN ((data->>'name') gin_trgm_ops); 