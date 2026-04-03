# TSP Solver

Solver del Problema del Vendedor Viajero (TSP) con interfaz web interactiva. Construido en Rust con un servidor axum, mapa Leaflet.js + OpenStreetMap, y optimización 2-opt con distancia Haversine.

## Uso

### Local

```bash
cargo run --release
# Abre http://localhost:3000
```

1. **Busca** una dirección en la barra de búsqueda, o **haz clic** en el mapa para agregar puntos
2. Agrega mínimo 3 puntos
3. Clic en **"Resolver TSP"** → dibuja la ruta óptima en rojo con la distancia total en km
4. **"Limpiar"** para empezar de nuevo

### Instalar en celular (PWA)

Con la app desplegada en Railway u otro hosting HTTPS:

1. Abre la URL en **Chrome** en tu Android
2. Menú (⋮) → **"Añadir a pantalla de inicio"**
3. Queda instalada como app nativa

### Deploy en Railway

1. Crea cuenta en [railway.app](https://railway.app)
2. "Deploy from GitHub repo" → selecciona este repositorio
3. Railway detecta Rust automáticamente y compila
4. Settings → Networking → **"Generate Domain"** para obtener URL pública

La app lee la variable de entorno `PORT` (inyectada por Railway) automáticamente.

## Arquitectura

```
Browser (Leaflet.js + OpenStreetMap)
    → click / búsqueda Nominatim para agregar puntos
    → POST /solve  →  axum server  →  tour.rs (Haversine + 2-opt)
    ← polyline roja ←  JSON { route, total_distance_km }
```

| Módulo | Responsabilidad |
|--------|----------------|
| `src/tour.rs` | Solver TSP: nearest-neighbour + 2-opt, distancia Haversine |
| `src/handlers.rs` | Handler HTTP `POST /solve` |
| `src/main.rs` | Servidor axum, sirve HTML/manifest/SW embebidos |
| `static/index.html` | UI: mapa Leaflet, buscador Nominatim, controles |

## Stack

- **Rust / axum** — servidor HTTP async
- **Leaflet.js + OpenStreetMap** — mapa (sin API key)
- **Nominatim** — geocodificación de direcciones (sin API key)
- **Haversine** — distancia real en km entre coordenadas lat/lng
- **PWA** — instalable en Android desde el navegador

## Dependencias

- [axum](https://crates.io/crates/axum) — servidor HTTP
- [tokio](https://crates.io/crates/tokio) — runtime async
- [serde / serde_json](https://crates.io/crates/serde) — serialización JSON
- [tower-http](https://crates.io/crates/tower-http) — middleware CORS
- [rand](https://crates.io/crates/rand) — generación aleatoria
