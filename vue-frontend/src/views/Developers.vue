<template>
  <div>
    <el-card shadow="hover">
      <template #header>
        <div style="display: flex; justify-content: space-between; align-items: center">
          <span>开发者列表</span>
          <div>
            <el-input
              v-model="searchQuery"
              placeholder="搜索开发者名称..."
              clearable
              style="width: 240px; margin-right: 12px"
              @input="handleSearch"
            />
            <el-button type="primary" @click="showCreateDialog">
              <el-icon><Plus /></el-icon> 新增开发者
            </el-button>
          </div>
        </div>
      </template>

      <el-table 
        :data="developers" 
        v-loading="loading" 
        stripe 
        border
        :scrollbar-always-on="false"
        :max-height="600"
      >
        <el-table-column prop="developer_name" label="名称" min-width="140">
          <template #default="{ row }">
            <router-link :to="`/developers/${row.developer_uuid}`" style="color: #409EFF; text-decoration: none">
              {{ row.developer_name }}
            </router-link>
          </template>
        </el-table-column>
        <el-table-column prop="developer_uuid" label="UUID" min-width="200" show-overflow-tooltip />
        <el-table-column prop="successful_auths" label="认证次数" width="110" sortable />
        <el-table-column prop="deduction_available" label="可扣款次数" width="120" sortable>
          <template #default="{ row }">
            <el-tag :type="row.deduction_available > 0 ? 'success' : 'warning'">
              {{ row.deduction_available }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="last_auth_time" label="最后认证时间" width="170">
          <template #default="{ row }">
            {{ formatTime(row.last_auth_time) }}
          </template>
        </el-table-column>
        <el-table-column prop="rate_limit_per_second" label="限流(次/秒)" width="120" sortable />
        <el-table-column prop="create_time" label="注册时间" width="170">
          <template #default="{ row }">
            {{ formatTime(row.create_time) }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="160" fixed="right">
          <template #default="{ row }">
            <el-button size="small" @click="editDev(row)">编辑</el-button>
            <el-button size="small" type="danger" @click="removeDev(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <div style="display: flex; justify-content: flex-end; margin-top: 16px">
        <el-pagination
          v-model:current-page="page"
          :page-size="pageSize"
          :total="total"
          layout="total, prev, pager, next, jumper"
          :page-sizes="[10, 20, 50, 100]"
          @size-change="handleSizeChange"
          @current-change="loadData"
        />
      </div>
    </el-card>

    <el-dialog v-model="dialogVisible" :title="isEdit ? '编辑开发者' : '新增开发者'" width="520px">
      <el-form :model="form" label-width="130px" ref="formRef">
        <el-form-item label="名称" required>
          <el-input v-model="form.developer_name" placeholder="开发者名称" />
        </el-form-item>
        <el-form-item label="UUID" v-if="!isEdit">
          <el-input v-model="form.developer_uuid" placeholder="留空自动生成" />
        </el-form-item>
        <el-form-item label="成功认证次数">
          <el-input-number v-model="form.successful_auths" :min="0" />
        </el-form-item>
        <el-form-item label="可扣款次数" required>
          <el-input-number v-model="form.deduction_available" :min="0" />
        </el-form-item>
        <el-form-item label="扣款上限" required>
          <el-input-number v-model="form.deduction_limit" :min="1" />
        </el-form-item>
        <el-form-item label="恢复数量/次">
          <el-input-number v-model="form.recovery_amount" :min="1" />
        </el-form-item>
        <el-form-item label="恢复间隔(秒)">
          <el-input-number v-model="form.recovery_interval_secs" :min="1" :max="3600" />
        </el-form-item>
        <el-form-item label="限流(次/秒)">
          <el-input-number v-model="form.rate_limit_per_second" :min="1" :max="10000" />
        </el-form-item>
        <el-form-item label="最后认证时间">
          <el-date-picker v-model="form.last_auth_time" type="datetime" placeholder="选择时间" format="YYYY-MM-DD HH:mm:ss" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitForm" :loading="submitting">{{ isEdit ? '更新' : '创建' }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { getDevelopers, createDeveloper, updateDeveloper, deleteDeveloper } from '../api'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'

const developers = ref([])
const loading = ref(false)
const page = ref(1)
const pageSize = ref(20)
const total = ref(0)
const searchQuery = ref('')
const dialogVisible = ref(false)
const isEdit = ref(false)
const submitting = ref(false)
const editingUuid = ref('')
let debounceTimer = null

const form = ref({
  developer_name: '',
  developer_uuid: '',
  successful_auths: 0,
  deduction_available: 0,
  deduction_limit: 1000,
  recovery_amount: 10,
  recovery_interval_secs: 60,
  rate_limit_per_second: 100,
  last_auth_time: null
})

function formatTime(t) {
  return t ? dayjs(t).format('YYYY-MM-DD HH:mm:ss') : '-'
}

async function loadData() {
  loading.value = true
  try {
    const params = { page: page.value, page_size: pageSize.value }
    if (searchQuery.value) params.search = searchQuery.value
    const res = await getDevelopers(params)
    developers.value = res.data?.data || []
    total.value = res.data?.total || 0
  } catch (error) {
    ElMessage.error('加载失败，请稍后重试')
  }
  loading.value = false
}

function handleSearch() {
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    page.value = 1
    loadData()
  }, 300)
}

function handleSizeChange(val) {
  pageSize.value = val
  page.value = 1
  loadData()
}

function showCreateDialog() {
  isEdit.value = false
  editingUuid.value = ''
  form.value = {
    developer_name: '',
    developer_uuid: '',
    successful_auths: 0,
    deduction_available: 0,
    deduction_limit: 1000,
    recovery_amount: 10,
    recovery_interval_secs: 60,
    rate_limit_per_second: 100,
    last_auth_time: null
  }
  dialogVisible.value = true
}

function editDev(dev) {
  isEdit.value = true
  editingUuid.value = dev.developer_uuid
  form.value = {
    developer_name: dev.developer_name,
    developer_uuid: dev.developer_uuid,
    successful_auths: dev.successful_auths,
    deduction_available: dev.deduction_available || 0,
    deduction_limit: dev.deduction_limit || 1000,
    recovery_amount: dev.recovery_amount || 10,
    recovery_interval_secs: dev.recovery_interval_secs || 60,
    rate_limit_per_second: dev.rate_limit_per_second || 100,
    last_auth_time: dev.last_auth_time ? new Date(dev.last_auth_time) : null
  }
  dialogVisible.value = true
}

async function submitForm() {
  if (!form.value.developer_name) {
    ElMessage.warning('请输入开发者名称')
    return
  }
  submitting.value = true
  try {
    const data = {
      ...form.value,
      last_auth_time: form.value.last_auth_time
        ? dayjs(form.value.last_auth_time).format('YYYY-MM-DD HH:mm:ss')
        : undefined
    }
    if (isEdit.value) {
      await updateDeveloper(editingUuid.value, data)
      ElMessage.success('更新成功')
    } else {
      await createDeveloper(data)
      ElMessage.success('创建成功')
    }
    dialogVisible.value = false
    loadData()
  } catch (error) {
    ElMessage.error('操作失败，请稍后重试')
  }
  submitting.value = false
}

async function removeDev(dev) {
  try {
    await ElMessageBox.confirm(`确定删除开发者 "${dev.developer_name}"？`, '确认', { type: 'warning' })
    await deleteDeveloper(dev.developer_uuid)
    ElMessage.success('删除成功')
    loadData()
  } catch {}
}

onMounted(loadData)

onUnmounted(() => {
  if (debounceTimer) clearTimeout(debounceTimer)
})
</script>