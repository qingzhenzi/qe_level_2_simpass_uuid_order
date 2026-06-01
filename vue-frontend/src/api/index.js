import axios from 'axios'
import { ElMessage } from 'element-plus'

const http = axios.create({
  baseURL: '/',
  timeout: 30000,
  headers: { 'Content-Type': 'application/json' }
})

// 从 localStorage 读取 Admin Token
const ADMIN_TOKEN_KEY = 'admin_token'
const DEFAULT_ADMIN_TOKEN = 'admin123secure'

// 自动添加 Admin Token 到请求头
http.interceptors.request.use(
  config => {
    const token = localStorage.getItem(ADMIN_TOKEN_KEY) || DEFAULT_ADMIN_TOKEN
    if (token) {
      config.headers['X-Admin-Token'] = token
    }
    return config
  },
  error => Promise.reject(error)
)

http.interceptors.response.use(
  response => {
    const data = response.data
    if (data.code && data.code !== 'SUCCESS') {
      ElMessage.error(data.message || 'Request failed')
      return Promise.reject(new Error(data.message))
    }
    return data
  },
  error => {
    const msg = error.response?.data?.message || error.message || 'Network error'
    ElMessage.error(msg)
    return Promise.reject(error)
  }
)

// 保存/获取 Admin Token 的辅助函数
export function setAdminToken(token) {
  localStorage.setItem(ADMIN_TOKEN_KEY, token)
}

export function getAdminToken() {
  return localStorage.getItem(ADMIN_TOKEN_KEY) || DEFAULT_ADMIN_TOKEN
}

export function getDevelopers(params) {
  return http.get('/api/developers', { params })
}

export function getDeveloper(uuid) {
  return http.get(`/api/developers/${uuid}`)
}

export function createDeveloper(data) {
  return http.post('/api/developers', data)
}

export function updateDeveloper(uuid, data) {
  return http.put(`/api/developers/${uuid}`, data)
}

export function deleteDeveloper(uuid) {
  return http.delete(`/api/developers/${uuid}`)
}

export function initiateDeduction(data) {
  return http.post('/api/deductions/initiate', data)
}

export function confirmDeduction(data) {
  return http.post('/api/deductions/confirm', data)
}

export function cancelDeduction(data) {
  return http.post('/api/deductions/cancel', data)
}

export function getTransactions(params) {
  return http.get('/api/deductions/transactions', { params })
}

export function getCurrentQps(params) {
  return http.get('/api/qps/current', { params })
}

export function getQpsHistory(params) {
  return http.get('/api/qps/history', { params })
}

export function getQpsStats() {
  return http.get('/api/qps/stats')
}

export default http
