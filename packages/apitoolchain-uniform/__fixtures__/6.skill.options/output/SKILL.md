---
name: pet-store-api
description: Reads and changes pets and orders in the Pet Store API
---

# Pet Store API

Every call needs `Authorization: Bearer <token>`.

| Prefix | Object |
|---|---|
| `pet_` | a pet |

Read [api/errors.md](api/errors.md) when a call fails.

## API reference

| Group | Operations | File |
|---|---|---|
| pets | 7 | [api/pets.md](api/pets.md) |
| Store / Inventory | 1 | [api/store-inventory.md](api/store-inventory.md) |
| store | 4 | [api/store.md](api/store.md) |

Every operation in one table: [api/index.md](api/index.md).

## Other files

- [api/errors.md](api/errors.md)
- [guides/pagination.md](guides/pagination.md)
