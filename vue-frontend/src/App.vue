<template>
  <el-container style="min-height: 100vh">
    <el-aside width="220px" style="background-color: #304156">
      <div class="logo">
        <h2 style="color: #fff; text-align: center; padding: 20px 0; margin: 0; font-size: 16px">
          SimPass UUID
        </h2>
      </div>
      <el-menu
        :default-active="currentRoute"
        router
        background-color="#304156"
        text-color="#bfcbd9"
        active-text-color="#409EFF"
        style="border-right: none"
      >
        <el-menu-item index="/">
          <el-icon><Monitor /></el-icon>
          <span>仪表盘</span>
        </el-menu-item>
        <el-menu-item index="/developers">
          <el-icon><User /></el-icon>
          <span>开发者管理</span>
        </el-menu-item>
        <el-menu-item index="/deductions">
          <el-icon><Coin /></el-icon>
          <span>扣款记录</span>
        </el-menu-item>
        <el-menu-item index="/qps">
          <el-icon><DataLine /></el-icon>
          <span>QPS监控</span>
        </el-menu-item>
      </el-menu>
    </el-aside>
    <el-container>
      <el-header style="background: #fff; border-bottom: 1px solid #e6e6e6; display: flex; align-items: center; justify-content: space-between">
        <span style="font-size: 16px; font-weight: 500">{{ pageTitle }}</span>
        <div style="display: flex; align-items: center; gap: 12px">
          <el-tag :type="healthStatus === 'healthy' ? 'success' : 'danger'" size="small">
            {{ healthStatus === 'healthy' ? '服务正常' : '服务异常' }}
          </el-tag>
          <el-button size="small" @click="showTokenDialog = true">
            {{ hasToken ? '更换 Token' : '设置 Token' }}
          </el-button>
        </div>
      </el-header>
      <el-main style="background-color: #f0f2f5">
        <router-view />
      </el-main>
    </el-container>

    <!-- Token 输入对话框 -->
    <el-dialog v-model="showTokenDialog" title="Admin Token" width="420px" :close-on-click-modal="false">
      <el-form label-width="100px">
        <el-form-item label="Admin Token">
          <el-input v-model="tokenInput" type="password" show-password placeholder="请输入 Admin Token" @keyup.enter="saveToken" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showTokenDialog = false">取消</el-button>
        <el-button type="primary" @click="saveToken" :disabled="!tokenInput.trim()">保存</el-button>
      </template>
    </el-dialog>
  </el-container>
</template>

<script setup>
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import axios from 'axios'
import { getAdminToken, setAdminToken } from './api'

const route = useRoute()
const healthStatus = ref('healthy')
const showTokenDialog = ref(false)
const tokenInput = ref('')
let healthTimer = null

const currentRoute = computed(() => route.path)

const hasToken = computed(() => !!getAdminToken())

const pageTitle = computed(() => {
  const titles = {
    '/': '仪表盘',
    '/developers': '开发者管理',
    '/deductions': '扣款记录',
    '/qps': 'QPS监控'
  }
  return titles[route.path] || 'SimPass UUID Order'
})

function saveToken() {
  if (tokenInput.value.trim()) {
    setAdminToken(tokenInput.value.trim())
    showTokenDialog = false
    tokenInput.value = ''
  }
}

async function checkHealth() {
  try {
    const res = await axios.get('/health')
    healthStatus.value = res.data.status
  } catch {
    healthStatus.value = 'unhealthy'
  }
}

onMounted(() => {
  checkHealth()
  healthTimer = setInterval(checkHealth, 10000)
})

onUnmounted(() => {
  if (healthTimer) clearInterval(healthTimer)
})
</script>

<style>
body {
  margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
}
</style>
