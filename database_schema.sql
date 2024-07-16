CREATE TABLE public.media (
	id varchar NOT NULL,
	"name" varchar NOT NULL,
	description text NOT NULL,
	upload int8 DEFAULT EXTRACT(epoch FROM now()) NOT NULL,
	"owner" varchar NOT NULL,
	likes int8 DEFAULT 0 NOT NULL,
	dislikes int8 DEFAULT 0 NOT NULL,
	"views" int8 DEFAULT 0 NOT NULL,
	public bool DEFAULT false NOT NULL,
	"type" varchar NOT NULL,
	CONSTRAINT videos_pk PRIMARY KEY (id)
);
CREATE TABLE public."comments" (
	id bigserial NOT NULL,
	video varchar NOT NULL,
	"user" varchar NOT NULL,
	"text" text NOT NULL,
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
CREATE TABLE public.subscribtions (
	subscriber varchar(40) NOT NULL,
	target varchar(40) NOT NULL,
);