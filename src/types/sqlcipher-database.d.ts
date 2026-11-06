declare module "better-sqlite3-multiple-ciphers" {
  interface Statement {
    get(...params: unknown[]): unknown;
    all(...params: unknown[]): unknown[];
    run(...params: unknown[]): { changes: number; lastInsertRowid: number | bigint };
  }

  interface SqliteDatabase {
    open: boolean;
    prepare(sql: string): Statement;
    exec(sql: string): this;
    pragma(source: string, options?: { simple?: boolean }): unknown;
    close(): this;
    key(key: Buffer): number;
    rekey(key: Buffer): number;
    transaction<T extends (...args: never[]) => void>(fn: T): T;
  }

  interface DatabaseConstructor {
    new (filename: string, options?: { fileMustExist?: boolean; readonly?: boolean }): SqliteDatabase;
  }

  const Database: DatabaseConstructor;
  export default Database;
}
