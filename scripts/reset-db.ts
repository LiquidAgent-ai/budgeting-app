import { seedDatabase } from "../src/lib/seed";

const result = seedDatabase(true);
console.log(result.message);
