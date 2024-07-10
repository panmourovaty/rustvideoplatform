# Rust Video Platform

## How to build
1. You need PostgreSQL database (versions 15+ are supported)
2. Run database_schema.sql on some database
3. create .env file for sqlx with DATABASE_URL=<your_database_connection_string>
4. install cargo (Rust 1.70+ should work)
5. run cargo build --release
6. you get binary

## How to run
1. You need PostgreSQL database (versions 15+ are supported)
2. Run database_schema.sql on some database
3. create folder "source" where will be folders, folder name = video id in database and each will include video.webm and thumbnail.avif
4. run your compiled binary
