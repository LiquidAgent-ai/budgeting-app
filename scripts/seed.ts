import { seedDatabase } from "../src/lib/seed";

const fresh = process.argv.includes("--fresh");
const result = seedDatabase(fresh);
console.log(result.message);
