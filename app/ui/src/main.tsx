// Atlas UI — точка входа интерфейса (раздел 7 ТЗ)
// Реализация интерфейса на этапе 2 (раздел 13.1)

import { createRoot } from 'react-dom/client'
import App from './App'

const container = document.getElementById('app')
if (container) {
  const root = createRoot(container)
  root.render(
    <div style={{ padding: '2rem', fontFamily: 'system-ui, sans-serif', color: '#333' }}>
      <h1>Atlas</h1>
      <p>Навигатор по проекту для нейросетей</p>
      <p>Интерфейс в разработке (этап 2, раздел 13.1 ТЗ)</p>
    </div>
  )
}