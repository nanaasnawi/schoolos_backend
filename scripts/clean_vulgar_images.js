const fs = require('fs');
const path = require('path');
const envText = fs.readFileSync(path.join(__dirname, '..', '.env'), 'utf8');
const m = envText.match(/^DATABASE_URL=(.+)$/m);
const dbUrl = m ? m[1].trim() : process.env.DATABASE_URL;
const { Client } = require('pg');

const SAFE_ENGLISH_IMAGE = 'https://images.unsplash.com/photo-1456513080510-7bf3a84b82f8?auto=format&fit=crop&w=1200&q=80';

(async () => {
  const client = new Client({ connectionString: dbUrl });
  await client.connect();

  const cols = await client.query("SELECT column_name FROM information_schema.columns WHERE table_name = 'learning_materials'");
  console.log('Columns:', cols.rows.map(r => r.column_name).join(', '));

  const targetRows = await client.query(`
    SELECT * FROM learning_materials 
    WHERE id IN ('01a129d8-0c44-7e72-8155-8353bad8c2c2', '01a129d5-4dd5-7331-b4e4-f03051a35ff0')
       OR title ILIKE '%Has dan Have%'
  `);

  console.log(`Found ${targetRows.rows.length} materials matching target.`);

  for (const row of targetRows.rows) {
    console.log('Checking material:', row.id, row.title);
    // Find text/json columns and replace vulgar or wikimedia URLs
    for (const key of Object.keys(row)) {
      const val = row[key];
      if (typeof val === 'string' && (val.includes('wikimedia') || val.includes('wikipedia') || val.includes('upload.wikimedia'))) {
        console.log(`Column ${key} has wikimedia content! Replacing...`);
        const cleaned = val.replace(/https:\/\/[^"'\s)]*(?:wikimedia\.org|wikipedia\.org)[^"'\s)]*/gi, SAFE_ENGLISH_IMAGE);
        await client.query(`UPDATE learning_materials SET ${key} = $1 WHERE id = $2`, [cleaned, row.id]);
        console.log(`Updated ${key} for ${row.id}`);
      } else if (typeof val === 'object' && val !== null) {
        const jsonStr = JSON.stringify(val);
        if (jsonStr.includes('wikimedia') || jsonStr.includes('wikipedia')) {
          console.log(`JSON Column ${key} has wikimedia content! Replacing...`);
          const cleanedStr = jsonStr.replace(/https:\/\/[^"'\s)]*(?:wikimedia\.org|wikipedia\.org)[^"'\s)]*/gi, SAFE_ENGLISH_IMAGE);
          const cleanedObj = JSON.parse(cleanedStr);
          await client.query(`UPDATE learning_materials SET ${key} = $1 WHERE id = $2`, [cleanedObj, row.id]);
          console.log(`Updated JSON ${key} for ${row.id}`);
        }
      }
    }
  }

  // Also check ALL learning_materials for any wikimedia or wikipedia image URLs
  console.log('--- Scanning entire table for any wikimedia links ---');
  const allRows = await client.query('SELECT * FROM learning_materials');
  for (const row of allRows.rows) {
    for (const key of Object.keys(row)) {
      const val = row[key];
      if (typeof val === 'string' && (val.includes('wikimedia.org') || val.includes('wikipedia.org'))) {
        console.log(`Found wikimedia in row ${row.id}, column ${key}`);
        const cleaned = val.replace(/https:\/\/[^"'\s)]*(?:wikimedia\.org|wikipedia\.org)[^"'\s)]*/gi, SAFE_ENGLISH_IMAGE);
        await client.query(`UPDATE learning_materials SET ${key} = $1 WHERE id = $2`, [cleaned, row.id]);
      } else if (typeof val === 'object' && val !== null) {
        const jsonStr = JSON.stringify(val);
        if (jsonStr.includes('wikimedia.org') || jsonStr.includes('wikipedia.org')) {
          console.log(`Found wikimedia in JSON row ${row.id}, column ${key}`);
          const cleanedStr = jsonStr.replace(/https:\/\/[^"'\s)]*(?:wikimedia\.org|wikipedia\.org)[^"'\s)]*/gi, SAFE_ENGLISH_IMAGE);
          await client.query(`UPDATE learning_materials SET ${key} = $1 WHERE id = $2`, [JSON.parse(cleanedStr), row.id]);
        }
      }
    }
  }

  await client.end();
  console.log('All vulgar / wikimedia images successfully cleansed from database!');
})();
