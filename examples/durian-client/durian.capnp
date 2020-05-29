@0xd767186f03554834;

struct Account {
  nonce @0: Data;
  balance @1: Data;
  code @2: Data;
}

struct Transaction {
  sender @0: Data;
  value @1: Data;
  gas @2: Data;
  gasPrice @3: Data;
  action: union{
    create: group {
      code @4: Data;
      salt @5: Data;
    }
    call: group {
      address @6: Data;
    }
  }
  args @7: Data;
}

struct LogEntry {
  address @0: Data;
  topics @1: List(Data);
  data @2: List(Int8);
}

struct ResultData {
  gasLeft @0: Data;
  data @1: Data;
  contract @2: Data;
  logs @3: List(LogEntry);
}

interface Executor {
  execute @0 (provider: Provider, transaction: Transaction) -> (resultData: ResultData);
}

interface Provider {
  exist @0          ( address: Data                             ) -> (exist: Bool);
  account @1        ( address: Data                             ) -> (account: Account);
  updateAccount @2  ( address: Data, balance: Data, nonce: Data ) -> ();
  createContract @3 ( address: Data, code: Data                 ) -> ();
  storageAt @4      ( address: Data, key: Data                  ) -> (storage: Data);
  setStorage @5     ( address: Data, key: Data, value: Data     ) -> ();
  timestamp @6      (                                           ) -> (timestamp: UInt64);
  blockNumber @7    (                                           ) -> (number: UInt64);
  blockHash @8      ( blockNo: UInt64                           ) -> (hash: Data);
  blockAuthor @9    (                                           ) -> (address: Data);
  difficulty @10    (                                           ) -> (difficulty: Data);
  gasLimit @11      (                                           ) -> (gasLimit: Data);
}
