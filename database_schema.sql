CREATE TABLE public.media (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	description jsonb,
	upload int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	"owner" varchar NOT NULL,
	"views" int8 DEFAULT 0 NOT NULL,
	visibility varchar DEFAULT 'hidden' NOT NULL,
	restricted_to_group varchar,
	"type" varchar NOT NULL,
	CONSTRAINT videos_pk PRIMARY KEY (id)
);
CREATE TABLE public."comments" (
	id bigserial NOT NULL,
	media varchar NOT NULL,
	"user" varchar NOT NULL,
	"text" jsonb NOT NULL,
	"time" int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	CONSTRAINT comments_pk PRIMARY KEY (id)
);
CREATE TABLE public.users (
	login varchar(40) NOT NULL,
	name varchar(100) NOT NULL,
	password_hash varchar NOT NULL,
	profile_picture varchar,
	channel_picture varchar,
	CONSTRAINT users_pk PRIMARY KEY (login)
);
CREATE TABLE public.subscriptions (
	subscriber varchar(40) NOT NULL,
	target varchar(40) NOT NULL
);
CREATE TABLE public.media_concepts (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	"owner" varchar NOT NULL,
	processed bool DEFAULT false NOT NULL,
	"type" varchar NOT NULL,
	CONSTRAINT media_concepts_pk PRIMARY KEY (id)
);
CREATE TABLE public.lists (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	"owner" varchar(40) NOT NULL,
	visibility varchar DEFAULT 'hidden' NOT NULL,
	restricted_to_group varchar,
	created int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	CONSTRAINT lists_pk PRIMARY KEY (id)
);
CREATE TABLE public.list_items (
	list_id varchar NOT NULL,
	media_id varchar NOT NULL,
	"position" int4 DEFAULT 0 NOT NULL,
	added int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL
);
CREATE TABLE public.user_groups (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	"owner" varchar(40) NOT NULL,
	created int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	CONSTRAINT user_groups_pk PRIMARY KEY (id)
);
CREATE TABLE public.user_group_members (
	group_id varchar NOT NULL,
	user_login varchar(40) NOT NULL,
	CONSTRAINT user_group_members_pk PRIMARY KEY (group_id, user_login)
);

CREATE TABLE public.media_likes (
	media_id varchar NOT NULL,
	user_login varchar(40) NOT NULL,
	reaction varchar NOT NULL,
	CONSTRAINT media_likes_pk PRIMARY KEY (media_id, user_login)
);

-- Migration from previous schema:
-- UPDATE public.media SET visibility = CASE WHEN public THEN 'public' ELSE 'hidden' END;
-- UPDATE public.lists SET visibility = CASE WHEN public THEN 'public' ELSE 'hidden' END;
-- ALTER TABLE public.media DROP COLUMN likes;
-- ALTER TABLE public.media DROP COLUMN dislikes;
-- ALTER TABLE public.media DROP COLUMN public;
-- ALTER TABLE public.lists DROP COLUMN public;
