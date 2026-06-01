<template>
  <div>
    <el-card shadow="hover">
      <template #header>
        <div style="display: flex; justify-content: space-between; align-items: center">
          <span>扣款交易记录</span>
          <div>
            <el-select v-model="statusFilter" placeholder="状态筛选" clearable style="width: 140px; margin-right: 12px" @change="loadData">
              <el-option label="待确认" value="pending" />
              <el-option label="已完成" value="committed" />
              <el-option label="已取消" value="cancelled" />
              <el-option label="已过期" value="expired" />
            </el-select>
            <el-input v-model="uuidFilter" placeholder="开发者UUID" clearable style="width: 240px; margin-right: 12px" @input="loadData" />
            <el-button type="primary" @click="loadData" :icon="'Refresh'" />
          </div>
        </div>
      </template>

      <el-table :data="transactions" v-loading="loading" stripe border>
        <el-table-column prop="transaction_token" label="交易Token" min-width="180" show-overflow-tooltip />
        <el-table-column prop="developer_uuid" label="开发者UUID" min-width="180" show-overflow-tooltip />
        <el-table-column prop="amount" label="数量" width="80" sortable />
        <el-table-column prop="status" label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="statusType(row.status)" size="small">{{ statusLabel(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间" width="170">
          <template #default="{ row }">{{ formatTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column prop="expires_at" label="过期时间" width="170">
          <template #default="{ row }">{{ formatTime(row.expires_at) }}</template>
        </el-table-column>
        <el-table-column prop="confirmed_at" label="确认时间" width="170">
          <template #default="{ row }">{{ formatTime(row.confirmed_at) }}</template>
        </el-table-column>
      </el-table>

      <div style="display: flex; justify-content: flex-end; margin-top: 16px">
        <el-pagination
          v-model:current-page="page"
          :page-size="20"
          :total="total"
          layout="total, prev, pager, next"
          @current-change="loadData"
        />
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getTransactions } from '../api'
import dayjs from 'dayjs'

const transactions = ref([])
const loading = ref(false)
const page = ref(1)
const total = ref(0)
const statusFilter = ref('')
const uuidFilter = ref('')

function statusType(s) {
  return { pending: 'warning', committed: 'success', cancelled: 'info', expired: 'danger' }[s] || 'info'
}
function statusLabel(s) {
  return { pending: '待确认', committed: '已完成', cancelled: '已取消', expired: '已过期' }[s] || s
}
function formatTime(t) {
  return t ? dayjs(t).format('YYYY-MM-DD HH:mm:ss') : '-'
}

async function loadData() {
  loading.value = true
  try {
    const params = { page: page.value, page_size: 20 }
    if (statusFilter.value) params.status = statusFilter.value
    if (uuidFilter.value) params.developer_uuid = uuidFilter.value
    const res = await getTransactions(params)
    transactions.value = res.data?.data || []
    total.value = res.data?.total || 0
  } catch {}
  loading.value = false
}

onMounted(loadData)
</script>
