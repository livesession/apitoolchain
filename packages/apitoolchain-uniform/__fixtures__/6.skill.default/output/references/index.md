<!-- Generated from the API reference. Do not edit by hand. -->

# Operations

| Operation | Method | Path | Group | Title |
|---|---|---|---|---|
| `listPets` | GET | `/pets` | [pets](pets.md) | List pets |
| `createPet` | POST | `/pets` | [pets](pets.md) | Create pet |
| `searchPets` | POST | `/pets/search` | [pets](pets.md) | Search pets |
| `getPet` | GET | `/pets/{petId}` | [pets](pets.md) | Retrieve pet |
| `updatePet` | PATCH | `/pets/{petId}` | [pets](pets.md) | Update pet |
| `deletePet` | DELETE | `/pets/{petId}` | [pets](pets.md) | Delete pet |
| `replacePetTags` | PUT | `/pets/{petId}/tags` | [pets](pets.md) | Replace pet tags |
| `listOwnerPets` | GET | `/owners/{name}/pets` | [pets](pets.md) | List owner pets |
| `uploadPetPhoto` | POST | `/pets/{petId}/photo` | [pets](pets.md) | Upload pet photo |
| `store-inventory` | GET | `/store/inventory` | [store](store.md) | Store inventory |
| `placeOrder` | POST | `/store/orders` | [store](store.md) | Place order |
| `shipOrder` | POST | `/store/orders/{id}/ship` | [store](store.md) | Ship order |
| `getOrder` | GET | `/store/orders/{id}` | [store](store.md) | Retrieve order |
| `replaceOrder` | PUT | `/store/orders/{id}` | [store](store.md) | Replace order |
| `cancelOrder` | DELETE | `/store/orders/{id}` | [store](store.md) | Cancel order |
| `createNote` | POST | `/notes` | [notes](notes.md) | createNote |
| `labelNote` | PUT | `/notes/{id}/labels` | [notes](notes.md) | labelNote |
| `patchNote` | PATCH | `/notes/{id}` | [notes](notes.md) | patchNote |
| `closeNote` | POST | `/notes/{id}/close` | [notes](notes.md) | closeNote |
| `get/health` | GET | `/health` | [general](general.md) | get/health |
