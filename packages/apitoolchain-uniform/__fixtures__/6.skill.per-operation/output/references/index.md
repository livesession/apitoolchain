<!-- Generated from the API reference. Do not edit by hand. -->

# Operations

| Operation | Method | Path | Group | Title |
|---|---|---|---|---|
| [`listPets`](pets/listPets.md) | GET | `/pets` | pets | List pets |
| [`createPet`](pets/createPet.md) | POST | `/pets` | pets | Create pet |
| [`searchPets`](pets/searchPets.md) | POST | `/pets/search` | pets | Search pets |
| [`getPet`](pets/getPet.md) | GET | `/pets/{petId}` | pets | Retrieve pet |
| [`updatePet`](pets/updatePet.md) | PATCH | `/pets/{petId}` | pets | Update pet |
| [`deletePet`](pets/deletePet.md) | DELETE | `/pets/{petId}` | pets | Delete pet |
| [`replacePetTags`](pets/replacePetTags.md) | PUT | `/pets/{petId}/tags` | pets | Replace pet tags |
| [`listOwnerPets`](pets/listOwnerPets.md) | GET | `/owners/{name}/pets` | pets | List owner pets |
| [`uploadPetPhoto`](pets/uploadPetPhoto.md) | POST | `/pets/{petId}/photo` | pets | Upload pet photo |
| [`store-inventory`](store/store-inventory.md) | GET | `/store/inventory` | store | Store inventory |
| [`placeOrder`](store/placeOrder.md) | POST | `/store/orders` | store | Place order |
| [`shipOrder`](store/shipOrder.md) | POST | `/store/orders/{id}/ship` | store | Ship order |
| [`getOrder`](store/getOrder.md) | GET | `/store/orders/{id}` | store | Retrieve order |
| [`replaceOrder`](store/replaceOrder.md) | PUT | `/store/orders/{id}` | store | Replace order |
| [`cancelOrder`](store/cancelOrder.md) | DELETE | `/store/orders/{id}` | store | Cancel order |
| [`createNote`](notes/createNote.md) | POST | `/notes` | notes | createNote |
| [`labelNote`](notes/labelNote.md) | PUT | `/notes/{id}/labels` | notes | labelNote |
| [`patchNote`](notes/patchNote.md) | PATCH | `/notes/{id}` | notes | patchNote |
| [`closeNote`](notes/closeNote.md) | POST | `/notes/{id}/close` | notes | closeNote |
| [`get/health`](general/get-health.md) | GET | `/health` | general | get/health |
