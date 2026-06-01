import { createRouter, createWebHistory } from 'vue-router'
import Dashboard from '../views/Dashboard.vue'
import Developers from '../views/Developers.vue'
import DeveloperDetail from '../views/DeveloperDetail.vue'
import Deductions from '../views/Deductions.vue'
import QpsMonitor from '../views/QpsMonitor.vue'

const routes = [
  { path: '/', name: 'Dashboard', component: Dashboard },
  { path: '/developers', name: 'Developers', component: Developers },
  { path: '/developers/:uuid', name: 'DeveloperDetail', component: DeveloperDetail },
  { path: '/deductions', name: 'Deductions', component: Deductions },
  { path: '/qps', name: 'QpsMonitor', component: QpsMonitor }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router
