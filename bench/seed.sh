#!/bin/bash
set -e

DB="blog.db"

# Seed 100 posts
echo "Seeding 100 posts..."
for i in $(seq 1 100); do
    sqlite3 "$DB" "INSERT OR IGNORE INTO posts (title, slug, content) VALUES ('Test Post $i', 'test-post-$i', '# Hello World $i

This is a **test post** with some markdown content.

- Item 1
- Item 2
- Item 3

\`\`\`rust
fn main() {
    println!(\"Hello from post $i\");
}
\`\`\`
');"
done
echo "Done seeding."
