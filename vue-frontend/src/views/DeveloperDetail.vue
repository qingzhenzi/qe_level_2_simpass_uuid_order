<template>
  <div>
    <el-card shadow="hover">
      <template #header>
        <div style="display: flex; justify-content: space-between; align-items: center">
          <span>开发者详情</span>
          <el-button @click="$router.push('/developers')">返回列表</el-button>
        </div>
      </template>

      <el-descriptions :column="2" border v-loading="loading">
        <el-descriptions-item label="开发者名称">{{ dev?.developer_name }}</el-descriptions-item>
        <el-descriptions-item label="UUID">{{ dev?.developer_uuid }}</el-descriptions-item>
        <el-descriptions-item label="成功认证次数">{{ dev?.successful_auths }}</el-descriptions-item>
        <el-descriptions-item label="可扣款次数">{{ dev?.deduction_available }}</el-descriptions-item>
        <el-descriptions-item label="扣款上限">{{ dev?.deduction_limit }}</el-descriptions-item>
        <el-descriptions-item label="恢复数量/次">{{ dev?.recovery_amount }}</el-descriptions-item>
        <el-descriptions-item label="恢复间隔(秒)">{{ dev?.recovery_interval_secs }}</el-descriptions-item>
        <el-descriptions-item label="注册时间">{{ formatTime(dev?.create_time) }}</el-descriptions-item>
        <el-descriptions-item label="最近认证时间">{{ formatTime(dev?.last_auth_time) }}</el-descriptions-item>
        <el-descriptions-item label="限流(请求/秒)">{{ dev?.rate_limit_per_second }}</el-descriptions-item>
      </el-descriptions>
    </el-card>

    <el-card shadow="hover" style="margin-top: 20px">
      <template #header><span>扣款操作</span></template>
      <el-form :model="deductForm" inline>
        <el-form-item label="扣款数量">
          <el-input-number v-model="deductForm.amount" :min="1" :max="dev?.deduction_available || 0" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="initiateDeduction" :loading="deducting">
            发起扣款
          </el-button>
        </el-form-item>
      </el-form>

      <div v-if="pendingTxn">
        <el-alert type="warning" show-icon :closable="false">
          <template #title>
            扣款待确认 - Token: {{ pendingTxn.transaction_token }} (有效期至 {{ formatTime(pendingTxn.expires_at) }})
          </template>
        </el-alert>
        <div style="margin-top: 12px">
          <el-button type="success" @click="confirmDeduction" :loading="confirming">确认扣款</el-button>
          <el-button type="danger" @click="cancelDeduction" :loading="cancelling">取消扣款</el-button>
        </div>
      </div>
    </el-card>

    <el-card shadow="hover" style="margin-top: 20px">
      <template #header><span>交易记录</span></template>
      <el-table :data="transactions" v-loading="loadingTxns" stripe border size="small">
        <el-table-column prop="transaction_token" label="交易Token" min-width="170" show-overflow-tooltip />
        <el-table-column prop="amount" label="数量" width="80" />
        <el-table-column prop="status" label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="statusType(row.status)" size="small">{{ statusLabel(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间" width="170">
          <template #default="{ row }">{{ formatTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column prop="confirmed_at" label="确认时间" width="170">
          <template #default="{ row }">{{ formatTime(row.confirmed_at) }}</template>
        </el-table-column>
      </el-table>
      <div style="display: flex; justify-content: flex-end; margin-top: 12px">
        <el-pagination
          v-model:current-page="txnPage"
          :page-size="10"
          :total="txnTotal"
          layout="total, prev, pager, next"
          size="small"
          @current-change="loadTransactions"
        />
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { getDeveloper, initiateDeduction as apiInitiate, confirmDeduction as apiConfirm, cancelDeduction as apiCancel, getTransactions } from '../api'
import { ElMessage } from 'element-plus'
import dayjs from 'dayjs'

const route = useRoute()
const loading = ref(false)
const dev = ref(null)
const deductForm = ref({ amount: 1 })
const deducting = ref(false)
const confirming = ref(false)
const cancelling = ref(false)
const pendingTxn = ref(null)
const transactions = ref([])
const loadingTxns = ref(false)
const txnPage = ref(1)
const txnTotal = ref(0)

function statusType(s) {
  return { pending: 'warning', committed: 'success', cancelled: 'info', expired: 'danger' }[s] || 'info'
}
function statusLabel(s) {
  return { pending: '待确认', committed: '已完成', cancelled: '已取消', expired: '已过期' }[s] || s
}

function formatTime(t) {
  return t ? dayjs(t).format('YYYY-MM-DD HH:mm:ss') : '-'
}

async function loadDev() {
  loading.value = true
  try {
    const res = await getDeveloper(route.params.uuid)
    dev.value = res.data
  } catch {}
  loading.value = false
}

async function loadTransactions() {
  loadingTxns.value = true
  try {
    const res = await getTransactions({ page: txnPage.value, page_size: 10, developer_uuid: route.params.uuid })
    transactions.value = res.data?.data || []
    txnTotal.value = res.data?.total || 0
  } catch {}
  loadingTxns.value = false
}

async function initiateDeduction() {
  deducting.value = true
  try {
    const res = await apiInitiate({ developer_uuid: route.params.uuid, amount: deductForm.value.amount })
    pendingTxn.value = res.data
    ElMessage.success('扣款请求已发起，请在30秒内确认')
    loadTransactions()
  } catch {}
  deducting.value = false
}

async function confirmDeduction() {
  if (!pendingTxn.value) return
  confirming.value = true
  try {
    await apiConfirm({
      transaction_token: pendingTxn.value.transaction_token,
      commit_token: pendingTxn.value.commit_token
    })
    ElMessage.success('扣款已确认')
    pendingTxn.value = null
    loadDev()
    loadTransactions()
  } catch {}
  confirming.value = false
}

async function cancelDeduction() {
  if (!pendingTxn.value) return
  cancelling.value = true
  try {
    await apiCancel({ transaction_token: pendingTxn.value.transaction_token })
    ElMessage.success('扣款已取消')
    pendingTxn.value = null
  } catch {}
  cancelling.value = false
}

onMounted(() => {
  loadDev()
  loadTransactions()
})
</script>
